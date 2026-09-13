//! Config preview command: generate effective core config for inspection.

use std::path::PathBuf;

use pp_client::config_slices::{ConfigSlices, ConfigSlicesStore};
use pp_client::{
    ClientConfig, EffectiveOverrides, PanelFeatures, SubscriptionStore, apply_panel_features,
    build_core_config_v2, compose_singbox_config, fetch_subscription_with_ua,
    inject_local_override_warn_only, resolve_remote_overrides,
};
use tauri::State;

use crate::commands::{cache_fetch_result, parse_subscription_id, sub_content_from_nodes};
use crate::state::AppState;

/// Generate effective config preview: subscription nodes -> template -> overrides -> core synthesis.
#[tauri::command]
pub async fn preview_core_config(
    state: State<'_, AppState>,
    subscription_id: Option<String>,
) -> Result<String, String> {
    let preview_id = match subscription_id.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(parse_subscription_id(s.trim())?),
        _ => None,
    };
    preview_core_config_impl(state.data_dir.clone(), preview_id).await
}

/// Implementation of config preview.
///
/// Pipeline mirrors `ClientState::start()`: config slices → profile overrides
/// (remote/local) → `build_core_config_v2` → `compose_singbox_config` →
/// `inject_local_override_warn_only` → `apply_panel_features`.
///
/// Known exception: the MITM chain is not injected here. Preview has no access
/// to the runtime `MitmChain` (it only exists once MITM is started in `start()`),
/// so `compose_singbox_config` is called with `None`. This is intentional and
/// does not change the returned schema.
pub(crate) async fn preview_core_config_impl(
    data_dir: PathBuf,
    preview_id: Option<uuid::Uuid>,
) -> Result<String, String> {
    let cfg = ClientConfig::load(&data_dir)
        .map_err(|e| format!("未找到已保存的配置（{e}），请先在设置页保存配置"))?;
    let cache_dir = data_dir.join("profile_cache");

    let sub_store = SubscriptionStore::new(data_dir.clone());
    let linked_profile_id: Option<uuid::Uuid>;
    let specified = match preview_id {
        Some(id) => {
            let subs = sub_store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
            Some(
                subs.iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| "订阅不存在".to_string())?
                    .clone(),
            )
        }
        None => None,
    };
    let sub_content = if let Some(sub) = &specified {
        linked_profile_id = sub.profile_id;
        if let Some(cached) = sub_store.load_cached_content(sub.id) {
            sub_content_from_nodes(&cached.singbox_nodes)
        } else {
            let fetch = fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
                .await
                .map_err(|e| format!("拉取订阅「{}」失败: {e}", sub.name))?;
            cache_fetch_result(&sub_store, sub.id, &fetch);
            sub_content_from_nodes(&fetch.singbox_nodes)
        }
    } else {
        match cfg.active_subscription_id {
            Some(id) => {
                let subs = sub_store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
                let sub = subs
                    .iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| "所选订阅不存在，请在首页重新选择".to_string())?;
                if !sub.enabled {
                    return Err("所选订阅已停用，请在订阅页启用或在首页重新选择".to_string());
                }
                linked_profile_id = sub.profile_id;
                if let Some(cached) = sub_store.load_cached_content(sub.id) {
                    sub_content_from_nodes(&cached.singbox_nodes)
                } else {
                    let fetch = fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
                        .await
                        .map_err(|e| format!("拉取订阅失败: {e}"))?;
                    cache_fetch_result(&sub_store, sub.id, &fetch);
                    sub_content_from_nodes(&fetch.singbox_nodes)
                }
            }
            None => return Err("请先在首页选择要使用的订阅".to_string()),
        }
    };

    let sub_name = specified.as_ref().map(|s| s.name.as_str());
    let store = pp_client::ProfileStoreV2::new(data_dir.clone());
    let (effective, warnings) = match linked_profile_id {
        Some(pid) => {
            let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
            let linked = profiles
                .iter()
                .find(|p| p.id == pid)
                .ok_or_else(|| match sub_name {
                    Some(name) => {
                        format!("订阅「{name}」关联的覆写模板不存在，请在订阅页重新关联")
                    }
                    None => "订阅关联的覆写模板不存在，请在订阅页重新关联".to_string(),
                })?;
            resolve_remote_overrides(&cache_dir, linked).await
        }
        None => (EffectiveOverrides::default(), Vec::new()),
    };
    for warning in &warnings {
        tracing::warn!(warning, "profile remote override");
    }

    // ⓪ Config slices (ADR-0005 §3.2/§3.4): load from `data_dir/config_slices.json`
    // and inject before profile overrides, mirroring `ClientState::start()`. A load
    // failure (unreadable/corrupted file is already handled inside the store) falls
    // back to default so preview never blocks.
    let slices = match ConfigSlicesStore::new(data_dir.clone()).load() {
        Ok(slices) => slices,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to load config_slices.json, falling back to default"
            );
            ConfigSlices::default()
        }
    };
    let profile_cfg = build_core_config_v2(&sub_content, &effective, &slices)
        .await
        .map_err(|e| format!("生成配置失败: {e}"))?;

    let features = PanelFeatures {
        tun_enabled: cfg.tun_enabled,
        tun_stack: cfg.tun_stack.clone(),
        tun_auto_route: cfg.tun_auto_route,
        clash_api_enabled: cfg.clash_api_enabled,
        clash_api_port: cfg.clash_api_port,
        clash_api_secret: cfg.clash_api_secret.clone(),
        clash_api_ui: cfg.clash_api_ui.clone(),
        rule_mode: cfg.normalized_rule_mode().to_string(),
        ipv6_enabled: cfg.ipv6_enabled,
        dns_fakeip_enabled: cfg.dns_fakeip_enabled,
        // FakeIP cache file pinned to the client data dir (explicit persistent path).
        data_dir: data_dir.to_string_lossy().into_owned(),
        // ADR-0005 D1: derive exactly like `ClientState::start()` so the preview
        // reflects the slices loaded above instead of a hardcoded default.
        dns_mode: pp_client::core_config::dns_mode_from_slices(&slices),
        github_proxy_prefix: cfg.github_proxy_prefix.clone(),
    };
    let mut value = compose_singbox_config(&profile_cfg, cfg.mixed_port, None)
        .map_err(|e| format!("合成 sing-box 配置失败: {e}"))?;
    // [ADR-0002] Inject local override after compose, before panel features,
    // mirroring `ClientState::start()`.
    inject_local_override_warn_only(&data_dir, &mut value);
    apply_panel_features(&mut value, &features);

    // 镜像 `ClientState::start()` 的规则集物化（只读本地已有文件、不触发下载）：
    // 预览反映当前本地规则集状态下的真实运行形态（缺失即降级后的配置）。
    let available = pp_client::ruleset_manager::existing_rule_sets(&data_dir);
    pp_client::ruleset_manager::materialize_rule_sets(&mut value, &data_dir, &available);

    serde_json::to_string_pretty(&value).map_err(|e| format!("序列化配置失败: {e}"))
}

#[cfg(test)]
#[path = "preview/tests.rs"]
mod tests;
