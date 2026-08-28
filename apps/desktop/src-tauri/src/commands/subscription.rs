//! Subscription management commands.

use std::path::PathBuf;

use pp_client::{
    fetch_subscription_with_ua, ClientConfig, Subscription, SubscriptionStore, SubContent,
    CachedSubscriptionContent,
};
use pp_common::CoreType;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::commands::{parse_profile_id, sub_format_str};
use crate::state::AppState;

/// Subscription user info view.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionUserInfoView {
    pub upload: Option<u64>,
    pub download: Option<u64>,
    pub total: Option<u64>,
    pub expire: Option<u64>,
}

impl SubscriptionUserInfoView {
    pub(crate) fn from_info(info: &pp_client::SubscriptionInfo) -> Self {
        Self {
            upload: info.upload,
            download: info.download,
            total: info.total,
            expire: info.expire,
        }
    }
}

/// Subscription view.
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub profile_id: Option<String>,
    pub userinfo: Option<SubscriptionUserInfoView>,
    pub node_count: u64,
    pub error: Option<String>,
    pub format: Option<String>,
    pub user_agent: Option<String>,
}

impl SubscriptionView {
    pub(crate) fn from_sub(sub: &Subscription) -> Self {
        Self {
            id: sub.id.to_string(),
            name: sub.name.clone(),
            url: sub.url.clone(),
            enabled: sub.enabled,
            profile_id: sub.profile_id.map(|v| v.to_string()),
            userinfo: sub
                .userinfo
                .as_ref()
                .map(SubscriptionUserInfoView::from_info),
            node_count: sub.node_count,
            error: sub.error.clone(),
            format: sub.format.map(sub_format_str).map(str::to_string),
            user_agent: sub.user_agent.clone(),
        }
    }
}

/// Input for adding a subscription.
#[derive(Debug, Deserialize)]
pub struct AddSubscriptionInput {
    pub name: String,
    pub url: String,
    pub user_agent: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
}

/// Parse subscription ID string to Uuid.
pub(crate) fn parse_subscription_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("无效的订阅 ID: {e}"))
}

/// Parse profile reference: None / empty = unlinked; non-empty must be valid Uuid.
pub(crate) fn parse_profile_ref(profile_id: &Option<String>) -> Result<Option<Uuid>, String> {
    match profile_id.as_deref() {
        Some(s) if !s.trim().is_empty() => Ok(Some(parse_profile_id(s.trim())?)),
        _ => Ok(None),
    }
}

/// Write fetch result to local content cache (best-effort).
pub(crate) fn cache_fetch_result(store: &SubscriptionStore, id: Uuid, fetch: &pp_client::FetchResult) {
    let cached = CachedSubscriptionContent {
        format: fetch.format,
        singbox_nodes: fetch.singbox_nodes.clone(),
        mihomo_nodes: fetch.mihomo_nodes.clone(),
    };
    if let Err(e) = store.write_cached_content(id, &cached) {
        tracing::warn!(id = %id, error = %e, "写入订阅内容缓存失败");
    }
}

/// Assemble SubContent from dual-core nodes.
pub(crate) fn sub_content_from_nodes(
    core_type: CoreType,
    singbox_nodes: &[serde_json::Value],
    mihomo_nodes: &[serde_json::Value],
) -> Result<SubContent, String> {
    match core_type {
        CoreType::SingBox => Ok(SubContent::SingBox(serde_json::json!({
            "outbounds": singbox_nodes,
        }))),
        CoreType::Mihomo => {
            let yaml = serde_yaml::to_string(&serde_json::json!({
                "proxies": mihomo_nodes,
            }))
            .map_err(|e| format!("序列化配置失败: {e}"))?;
            Ok(SubContent::Mihomo(yaml))
        }
    }
}

/// Apply fetch result to subscription (updates userinfo / node count, clears error on success).
pub(crate) async fn apply_fetch(store: &SubscriptionStore, sub: &mut Subscription, url: &str) {
    match fetch_subscription_with_ua(url, sub.user_agent.as_deref()).await {
        Ok(result) => {
            sub.node_count = result.singbox_nodes.len() as u64;
            sub.format = Some(result.format);
            sub.error = None;
            cache_fetch_result(store, sub.id, &result);
            sub.userinfo = result.userinfo;
        }
        Err(e) => {
            sub.error = Some(format!("拉取失败: {e}"));
        }
    }
}

/// Write updated subscription back to store.
pub(crate) fn write_subscription(store: &SubscriptionStore, sub: &Subscription) -> Result<(), String> {
    let mut subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    if let Some(existing) = subs.iter_mut().find(|s| s.id == sub.id) {
        *existing = sub.clone();
    }
    store.save(&subs).map_err(|e| format!("保存订阅失败: {e}"))
}

/// List all subscriptions.
#[tauri::command]
pub async fn list_subscriptions(
    state: State<'_, AppState>,
) -> Result<Vec<SubscriptionView>, String> {
    let store = SubscriptionStore::new(state.data_dir.clone());
    let subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    Ok(subs.iter().map(SubscriptionView::from_sub).collect())
}

/// Add subscription: validate URL -> persist -> immediately fetch once.
#[tauri::command]
pub async fn add_subscription(
    state: State<'_, AppState>,
    input: AddSubscriptionInput,
) -> Result<SubscriptionView, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let url = input.url.trim().to_string();
    pp_client::validate_subscription_url(&url)
        .map_err(|e| format!("订阅 URL 校验失败: {e}"))?;
    let ua = input
        .user_agent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let profile_id = parse_profile_ref(&input.profile_id)?;

    let store = SubscriptionStore::new(state.data_dir.clone());
    let mut sub = store
        .add(&name, &url, true, ua)
        .map_err(|e| format!("保存订阅失败: {e}"))?;
    sub.profile_id = profile_id;
    apply_fetch(&store, &mut sub, &url).await;
    write_subscription(&store, &sub)?;
    Ok(SubscriptionView::from_sub(&sub))
}

/// Refresh subscription: re-fetch to update userinfo / node count.
#[tauri::command]
pub async fn refresh_subscription(
    state: State<'_, AppState>,
    id: String,
) -> Result<SubscriptionView, String> {
    let id = parse_subscription_id(&id)?;
    let store = SubscriptionStore::new(state.data_dir.clone());
    let mut subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    let idx = subs
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| "订阅不存在".to_string())?;
    let url = subs[idx].url.clone();
    apply_fetch(&store, &mut subs[idx], &url).await;
    store
        .save(&subs)
        .map_err(|e| format!("保存订阅失败: {e}"))?;
    Ok(SubscriptionView::from_sub(&subs[idx]))
}

/// Remove subscription; silently returns if not found.
#[tauri::command]
pub async fn remove_subscription(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_subscription_id(&id)?;
    let store = SubscriptionStore::new(state.data_dir.clone());
    store.remove(id).map_err(|e| format!("删除订阅失败: {e}"))
}

/// Toggle subscription enabled state.
#[tauri::command]
pub async fn set_subscription_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let id = parse_subscription_id(&id)?;
    set_subscription_enabled_impl(&state.data_dir, id, enabled)
}

/// Implementation of `set_subscription_enabled`.
pub(crate) fn set_subscription_enabled_impl(
    data_dir: &std::path::Path,
    id: Uuid,
    enabled: bool,
) -> Result<(), String> {
    let store = SubscriptionStore::new(data_dir.to_path_buf());
    store
        .set_enabled(id, enabled)
        .map_err(|e| format!("保存订阅失败: {e}"))?;
    if !enabled {
        if let Ok(mut config) = ClientConfig::load(data_dir) {
            if config.active_subscription_id == Some(id) {
                config.active_subscription_id = None;
                config.save().map_err(|e| format!("保存配置失败: {e}"))?;
            }
        }
    }
    Ok(())
}

/// Set active subscription (home page selection).
#[tauri::command]
pub async fn set_active_subscription(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<(), String> {
    set_active_subscription_impl(&state.data_dir, id)
}

/// Implementation of `set_active_subscription`.
pub(crate) fn set_active_subscription_impl(
    data_dir: &std::path::Path,
    id: Option<String>,
) -> Result<(), String> {
    let mut config = match ClientConfig::load(data_dir) {
        Ok(cfg) => cfg,
        Err(_) => ClientConfig::new(
            data_dir.to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        ),
    };
    match id {
        Some(id) => {
            let id = parse_subscription_id(&id)?;
            let store = SubscriptionStore::new(data_dir.to_path_buf());
            let subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
            let sub = subs
                .iter()
                .find(|s| s.id == id)
                .ok_or_else(|| "所选订阅不存在".to_string())?;
            if !sub.enabled {
                return Err("所选订阅已停用，请先在订阅页启用".to_string());
            }
            config.active_subscription_id = Some(id);
        }
        None => {
            config.active_subscription_id = None;
        }
    }
    config.save().map_err(|e| format!("保存配置失败: {e}"))
}

/// Update subscription name / url / user_agent / profile link.
#[tauri::command]
pub async fn update_subscription(
    state: State<'_, AppState>,
    id: String,
    name: String,
    url: String,
    user_agent: Option<String>,
    profile_id: Option<String>,
) -> Result<SubscriptionView, String> {
    let id = parse_subscription_id(&id)?;
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let url = pp_client::normalize_resource_url(url.trim());
    pp_client::validate_subscription_url(&url)
        .map_err(|e| format!("订阅 URL 校验失败: {e}"))?;
    let ua = user_agent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let profile_id = parse_profile_ref(&profile_id)?;

    let store = SubscriptionStore::new(state.data_dir.clone());
    store
        .update(id, &name, &url, ua)
        .map_err(|e| format!("更新订阅失败: {e}"))?;
    store
        .set_profile_id(id, profile_id)
        .map_err(|e| format!("更新订阅失败: {e}"))?;
    let subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    let sub = subs
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "订阅不存在".to_string())?;
    Ok(SubscriptionView::from_sub(sub))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestDir(std::path::PathBuf);

    impl TestDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pp-client-ui-test-{}-{}",
                std::process::id(),
                n
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn parse_profile_ref_maps_empty_or_none_to_none_and_parses_uuid() {
        assert_eq!(parse_profile_ref(&None).unwrap(), None);
        assert_eq!(parse_profile_ref(&Some(String::new())).unwrap(), None);
        assert_eq!(parse_profile_ref(&Some("  ".to_string())).unwrap(), None);
        let id = Uuid::new_v4();
        assert_eq!(parse_profile_ref(&Some(id.to_string())).unwrap(), Some(id));
        assert!(parse_profile_ref(&Some("not-a-uuid".to_string())).is_err());
    }

    #[test]
    fn subscription_view_exposes_profile_id() {
        let sub = Subscription {
            id: Uuid::new_v4(),
            name: "sub".to_string(),
            url: "https://example.com/sub".to_string(),
            enabled: true,
            userinfo: None,
            node_count: 0,
            error: None,
            user_agent: None,
            format: None,
            profile_id: Some(Uuid::new_v4()),
        };
        let view = SubscriptionView::from_sub(&sub);
        assert_eq!(view.profile_id, sub.profile_id.map(|v| v.to_string()));

        let mut sub = sub;
        sub.profile_id = None;
        let view = SubscriptionView::from_sub(&sub);
        assert_eq!(view.profile_id, None);
    }

    #[test]
    fn set_active_subscription_validates_and_persists_selection() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let err = set_active_subscription_impl(dir.path(), Some(Uuid::new_v4().to_string())).unwrap_err();
        assert!(err.contains("不存在"), "{err}");

        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let off = store.add("off", "https://example.com/sub", false, None).unwrap();
        let err = set_active_subscription_impl(dir.path(), Some(off.id.to_string())).unwrap_err();
        assert!(err.contains("已停用"), "{err}");

        let on = store.add("on", "https://example.com/sub2", true, None).unwrap();
        set_active_subscription_impl(dir.path(), Some(on.id.to_string())).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, Some(on.id));

        set_active_subscription_impl(dir.path(), None).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, None);
    }

    #[test]
    fn disabling_selected_subscription_clears_active_selection() {
        let dir = TestDir::new();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store.add("sub", "https://example.com/sub", true, None).unwrap();
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.save().unwrap();

        set_subscription_enabled_impl(dir.path(), sub.id, false).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, None);

        set_subscription_enabled_impl(dir.path(), sub.id, true).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, None);

        let other = store.add("other", "https://example.com/other", false, None).unwrap();
        let mut cfg = ClientConfig::load(dir.path()).unwrap();
        cfg.active_subscription_id = Some(sub.id);
        cfg.save().unwrap();
        set_subscription_enabled_impl(dir.path(), other.id, false).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, Some(sub.id));
    }
}
