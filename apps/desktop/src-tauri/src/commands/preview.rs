//! Config preview command: generate effective core config for inspection.

use std::path::PathBuf;

use pp_client::{
    apply_panel_features, build_core_config_v2, compose_mihomo_config, compose_singbox_config,
    fetch_subscription_with_ua, resolve_remote_overrides, ClientConfig, EffectiveOverrides,
    PanelFeatures, SubscriptionFetcher, SubscriptionStore, SubContent, SubFormat,
};
use pp_common::CoreType;
use tauri::State;

use crate::commands::{sub_content_from_nodes, cache_fetch_result, parse_subscription_id};
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
pub(crate) async fn preview_core_config_impl(
    data_dir: PathBuf,
    preview_id: Option<uuid::Uuid>,
) -> Result<String, String> {
    let cfg = ClientConfig::load(&data_dir)
        .map_err(|e| format!("未找到已保存的配置（{e}），请先在设置页保存配置"))?;
    let cache_dir = data_dir.join("profile_cache");

    let sub_store = SubscriptionStore::new(data_dir.clone());
    let mut linked_profile_id = None;
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
            check_preview_core_compat(cached.format, cfg.core_type)
                .map_err(|e| format!("订阅「{}」无法预览: {e}", sub.name))?;
            sub_content_from_nodes(cfg.core_type, &cached.singbox_nodes, &cached.mihomo_nodes)?
        } else {
            let fetch = fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
                .await
                .map_err(|e| format!("拉取订阅「{}」失败: {e}", sub.name))?;
            check_preview_core_compat(fetch.format, cfg.core_type)
                .map_err(|e| format!("订阅「{}」无法预览: {e}", sub.name))?;
            cache_fetch_result(&sub_store, sub.id, &fetch);
            sub_content_from_nodes(cfg.core_type, &fetch.singbox_nodes, &fetch.mihomo_nodes)?
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
                    sub_content_from_nodes(
                        cfg.core_type,
                        &cached.singbox_nodes,
                        &cached.mihomo_nodes,
                    )?
                } else {
                    let fetch = fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
                        .await
                        .map_err(|e| format!("拉取订阅失败: {e}"))?;
                    cache_fetch_result(&sub_store, sub.id, &fetch);
                    sub_content_from_nodes(
                        cfg.core_type,
                        &fetch.singbox_nodes,
                        &fetch.mihomo_nodes,
                    )?
                }
            }
            None if !cfg.hub_url.is_empty() && !cfg.sub_token.is_empty() => {
                let fetcher = SubscriptionFetcher::new();
                match cfg.core_type {
                    CoreType::SingBox => {
                        let (config, _) = fetcher
                            .fetch_singbox_config(&cfg.hub_url, &cfg.sub_token)
                            .await
                            .map_err(|e| format!("拉取订阅失败: {e}"))?;
                        SubContent::SingBox(config)
                    }
                    CoreType::Mihomo => {
                        let (yaml, _) = fetcher
                            .fetch_clash_config(&cfg.hub_url, &cfg.sub_token)
                            .await
                            .map_err(|e| format!("拉取订阅失败: {e}"))?;
                        SubContent::Mihomo(yaml)
                    }
                }
            }
            _ => return Err("请先在首页选择要使用的订阅".to_string()),
        }
    };

    let sub_name = specified.as_ref().map(|s| s.name.as_str());
    let store = pp_client::ProfileStoreV2::new(data_dir);
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
            if linked.core_type != cfg.core_type {
                return Err(match sub_name {
                    Some(name) => format!(
                        "订阅「{name}」关联的覆写模板「{}」适用于 {}，与当前核心 {} 不匹配，请在首页切换核心或在订阅页调整关联",
                        linked.name,
                        pp_client::core_type_display_name(linked.core_type),
                        pp_client::core_type_display_name(cfg.core_type),
                    ),
                    None => format!(
                        "覆写模板「{}」适用于 {}，与当前核心 {} 不匹配，请在首页切换核心或在订阅页调整关联",
                        linked.name,
                        pp_client::core_type_display_name(linked.core_type),
                        pp_client::core_type_display_name(cfg.core_type),
                    ),
                });
            }
            resolve_remote_overrides(&cache_dir, linked).await
        }
        None => (EffectiveOverrides::default(), Vec::new()),
    };
    for warning in &warnings {
        tracing::warn!(warning, "profile remote override");
    }

    let profile_cfg = build_core_config_v2(cfg.core_type, &sub_content, &effective)
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
    };
    let mut value = match cfg.core_type {
        CoreType::SingBox => compose_singbox_config(&profile_cfg, cfg.mixed_port, None)
            .map_err(|e| format!("合成 sing-box 配置失败: {e}"))?,
        CoreType::Mihomo => {
            let yaml =
                serde_yaml::to_string(&profile_cfg).map_err(|e| format!("序列化配置失败: {e}"))?;
            compose_mihomo_config(&yaml, cfg.mixed_port, None)
                .map_err(|e| format!("合成 mihomo 配置失败: {e}"))?
        }
    };
    apply_panel_features(&mut value, cfg.core_type, &features);

    match cfg.core_type {
        CoreType::SingBox => {
            serde_json::to_string_pretty(&value).map_err(|e| format!("序列化配置失败: {e}"))
        }
        CoreType::Mihomo => {
            serde_yaml::to_string(&value).map_err(|e| format!("序列化配置失败: {e}"))
        }
    }
}

/// Subscription format <-> core type compatibility check.
pub(crate) fn check_preview_core_compat(format: SubFormat, core_type: CoreType) -> Result<(), String> {
    let compatible = match format {
        SubFormat::ShareLinks => true,
        SubFormat::SingBoxJson => core_type == CoreType::SingBox,
        SubFormat::ClashYaml => core_type == CoreType::Mihomo,
    };
    if compatible {
        return Ok(());
    }
    let (format_name, supported_core) = if format == SubFormat::ClashYaml {
        ("clash", "mihomo")
    } else {
        ("sing-box", "sing-box")
    };
    Err(format!(
        "订阅格式为 {format_name}，仅支持 {supported_core} 核心，当前核心类型为 {core_type}，请在设置中切换核心类型"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    struct TestDir(PathBuf);

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

    fn spawn_sub_server(body: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue; };
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{addr}")
    }

    const PREVIEW_SUB_JSON: &str = r#"{
        "outbounds": [
            { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
              "uuid": "12345678-1234-1234-1234-123456789012",
              "tls": { "enabled": true, "server_name": "example.com" } }
        ]
    }"#;

    #[tokio::test]
    async fn preview_core_config_specified_subscription_generates_config() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let base = spawn_sub_server(PREVIEW_SUB_JSON);
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store.add("spec", &format!("{base}/sub"), false, None).unwrap();

        let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
            .await
            .expect("specified subscription preview should succeed");
        let value: serde_json::Value = serde_json::from_str(&text).expect("sing-box preview should be JSON");
        assert!(value.get("outbounds").is_some());
        assert!(value.get("inbounds").is_some());
    }

    #[tokio::test]
    async fn preview_core_config_specified_unknown_subscription_errors() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let err = preview_core_config_impl(dir.path().to_path_buf(), Some(uuid::Uuid::new_v4()))
            .await
            .unwrap_err();
        assert!(err.contains("订阅不存在"), "{err}");
    }

    #[tokio::test]
    async fn preview_core_config_none_uses_active_subscription_selection() {
        let dir = TestDir::new();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let off = store.add("off", "https://example.com/sub", false, None).unwrap();
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.active_subscription_id = Some(off.id);
        cfg.save().unwrap();

        let err = preview_core_config_impl(dir.path().to_path_buf(), None)
            .await
            .unwrap_err();
        assert!(err.contains("已停用"), "{err}");

        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();
        let err = preview_core_config_impl(dir.path().to_path_buf(), None)
            .await
            .unwrap_err();
        assert!(err.contains("请先在首页选择要使用的订阅"), "{err}");
    }

    #[tokio::test]
    async fn preview_core_config_specified_uses_local_cache_without_network() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store.add("spec", "http://127.0.0.1:1/unreachable", false, None).unwrap();
        store.write_cached_content(
            sub.id,
            &CachedSubscriptionContent {
                format: SubFormat::SingBoxJson,
                singbox_nodes: vec![serde_json::json!({
                    "type": "vless",
                    "tag": "n1",
                    "server": "example.com",
                    "server_port": 443,
                    "uuid": "12345678-1234-1234-1234-123456789012",
                    "tls": { "enabled": true, "server_name": "example.com" },
                })],
                mihomo_nodes: Vec::new(),
            },
        ).unwrap();

        let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
            .await
            .expect("cached preview should succeed");
        let value: serde_json::Value = serde_json::from_str(&text).expect("sing-box preview should be JSON");
        assert!(value.get("outbounds").is_some());
    }

    #[tokio::test]
    async fn preview_core_config_specified_fallback_writes_cache() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let base = spawn_sub_server(PREVIEW_SUB_JSON);
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store.add("spec", &format!("{base}/sub"), false, None).unwrap();

        let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
            .await
            .expect("fallback fetch preview should succeed");
        let value: serde_json::Value = serde_json::from_str(&text).expect("sing-box preview should be JSON");
        assert!(value.get("outbounds").is_some());
        let cached = store.load_cached_content(sub.id).expect("fallback should write cache");
        assert_eq!(cached.format, SubFormat::SingBoxJson);
        assert!(!cached.singbox_nodes.is_empty());
    }
}
