//! Configuration commands: read / save client settings.

use std::path::PathBuf;

use pp_client::ClientConfig;
use pp_common::CoreType;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// External view of client config (serde simple struct, avoids exposing internal types).
///
/// All `*View` structs serialize field names as-is (snake_case) to align with
/// frontend `src/api.ts` TS types. `#[serde(default)]` backfills missing fields
/// from old frontend payloads to avoid deserialization failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfigView {
    /// Data directory (display only; persistence path is determined by app state).
    pub data_dir: String,
    pub hub_url: String,
    pub sub_token: String,
    /// Active subscription selected on the home page (`null` = none selected).
    pub active_subscription_id: Option<String>,
    /// Core type: `singbox` / `mihomo`.
    pub core_type: String,
    pub core_binary: String,
    pub mixed_port: u16,
    pub mitm_enabled: bool,
    pub mitm_hostnames: Vec<String>,
    /// MITM script dialect: `Surge` / `Loon`.
    pub mitm_script_dialect: String,
    pub system_proxy_enabled: bool,
    /// Whether TUN virtual interface is enabled (requires root / admin).
    pub tun_enabled: bool,
    /// TUN stack: `gvisor` / `system` / `mixed`.
    pub tun_stack: String,
    /// TUN auto route.
    pub tun_auto_route: bool,
    /// Whether Clash dashboard API is enabled.
    pub clash_api_enabled: bool,
    /// Clash dashboard API listen port.
    pub clash_api_port: u16,
    /// Clash dashboard API secret (empty = no auth).
    pub clash_api_secret: String,
    /// Clash dashboard UI selection: `yacd` / `zashboard` / `metacubexd` (default `zashboard`).
    pub clash_api_ui: String,
    /// GitHub proxy prefix (e.g. `https://gh-proxy.com`; empty = direct).
    pub github_proxy_prefix: String,
    /// Whether remote resource fetching goes through local core mixed port proxy.
    pub fetch_via_local_proxy: bool,
    /// Rule mode: `rule` / `global` / `direct` (default `rule`).
    ///
    /// Invalid values are persisted as-is and normalized on read by
    /// `pp_client::ClientConfig::normalized_rule_mode()`.
    pub rule_mode: String,
    /// Whether to show upload/download traffic in the Android VPN notification.
    pub vpn_notify_show_traffic: bool,
    /// Whether to show current proxy group & node in the Android VPN notification.
    pub vpn_notify_show_selection: bool,
}

impl Default for ClientConfigView {
    fn default() -> Self {
        Self {
            data_dir: String::new(),
            hub_url: String::new(),
            sub_token: String::new(),
            active_subscription_id: None,
            core_type: "singbox".to_string(),
            core_binary: String::new(),
            mixed_port: 17890,
            mitm_enabled: true,
            mitm_hostnames: Vec::new(),
            mitm_script_dialect: "Surge".to_string(),
            system_proxy_enabled: false,
            tun_enabled: false,
            tun_stack: "mixed".to_string(),
            tun_auto_route: true,
            clash_api_enabled: false,
            clash_api_port: 9090,
            clash_api_secret: String::new(),
            clash_api_ui: "zashboard".to_string(),
            github_proxy_prefix: String::new(),
            fetch_via_local_proxy: false,
            rule_mode: "rule".to_string(),
            vpn_notify_show_traffic: true,
            vpn_notify_show_selection: true,
        }
    }
}

impl ClientConfigView {
    /// Constructs view from internal config.
    pub(crate) fn from_config(cfg: &ClientConfig) -> Self {
        Self {
            data_dir: cfg.data_dir.to_string_lossy().into_owned(),
            hub_url: cfg.hub_url.clone(),
            sub_token: cfg.sub_token.clone(),
            active_subscription_id: cfg.active_subscription_id.map(|v| v.to_string()),
            core_type: serde_json::to_value(cfg.core_type)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            core_binary: cfg.core_binary.to_string_lossy().into_owned(),
            mixed_port: cfg.mixed_port,
            mitm_enabled: cfg.mitm_enabled,
            mitm_hostnames: cfg.mitm.hostnames.clone(),
            mitm_script_dialect: serde_json::to_value(cfg.mitm.script_dialect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            system_proxy_enabled: cfg.system_proxy_enabled,
            tun_enabled: cfg.tun_enabled,
            tun_stack: cfg.tun_stack.clone(),
            tun_auto_route: cfg.tun_auto_route,
            clash_api_enabled: cfg.clash_api_enabled,
            clash_api_port: cfg.clash_api_port,
            clash_api_secret: cfg.clash_api_secret.clone(),
            clash_api_ui: cfg.clash_api_ui.clone(),
            github_proxy_prefix: cfg.github_proxy_prefix.clone(),
            fetch_via_local_proxy: cfg.fetch_via_local_proxy,
            rule_mode: cfg.rule_mode.clone(),
            vpn_notify_show_traffic: cfg.vpn_notify_show_traffic,
            vpn_notify_show_selection: cfg.vpn_notify_show_selection,
        }
    }

    /// Converts to internal config; `data_dir` is supplied by app state.
    pub(crate) fn into_config(self, data_dir: &std::path::Path) -> Result<ClientConfig, String> {
        let value = serde_json::json!({
            "data_dir": data_dir,
            "hub_url": self.hub_url,
            "sub_token": self.sub_token,
            "active_subscription_id": self.active_subscription_id,
            "core_type": self.core_type,
            "core_binary": self.core_binary,
            "mixed_port": self.mixed_port,
            "mitm_enabled": self.mitm_enabled,
            "mitm": {
                "ca_dir": data_dir.join("certs"),
                "hostnames": self.mitm_hostnames,
                "script_dialect": self.mitm_script_dialect,
            },
            "system_proxy_enabled": self.system_proxy_enabled,
            "tun_enabled": self.tun_enabled,
            "tun_stack": self.tun_stack,
            "tun_auto_route": self.tun_auto_route,
            "clash_api_enabled": self.clash_api_enabled,
            "clash_api_port": self.clash_api_port,
            "clash_api_secret": self.clash_api_secret,
            "clash_api_ui": self.clash_api_ui,
            "github_proxy_prefix": self.github_proxy_prefix,
            "fetch_via_local_proxy": self.fetch_via_local_proxy,
            "rule_mode": self.rule_mode,
            "vpn_notify_show_traffic": self.vpn_notify_show_traffic,
            "vpn_notify_show_selection": self.vpn_notify_show_selection,
        });
        serde_json::from_value::<ClientConfig>(value).map_err(|e| e.to_string())
    }
}

/// Read current config (`data_dir/client.json` missing => return defaults).
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ClientConfigView, String> {
    let config_path = state.data_dir.join("client.json");
    let cfg = if config_path.exists() {
        ClientConfig::load(&state.data_dir).map_err(|e| format!("读取配置失败: {e}"))?
    } else {
        ClientConfig::new(
            state.data_dir.clone(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        )
    };
    Ok(ClientConfigView::from_config(&cfg))
}

/// Save config response view (carries non-blocking warnings).
#[derive(Debug, Clone, Serialize, Default)]
pub struct SaveConfigView {
    /// Non-blocking warning (e.g. missing local core after core_type switch).
    pub warning: Option<String>,
}

/// Save config implementation (testable pure logic at command layer).
///
/// - Basic settings can always be saved;
/// - `core_type` change auto-links local core binary (desktop only; Android
///   core is built-in panelcore, skip linkage to avoid misleading warnings).
pub(crate) fn save_config_impl(
    data_dir: &std::path::Path,
    cfg: ClientConfigView,
) -> Result<SaveConfigView, String> {
    #[cfg_attr(target_os = "android", allow(unused_mut))]
    let mut config = cfg.into_config(data_dir)?;
    #[cfg_attr(target_os = "android", allow(unused_mut))]
    let mut warnings: Vec<String> = Vec::new();

    // Android: ignore frontend-submitted core_type changes.
    #[cfg(target_os = "android")]
    {
        if let Ok(prev) = ClientConfig::load(data_dir) {
            if prev.core_type != config.core_type {
                tracing::warn!(
                    "Ignoring frontend core_type change on Android: {:?} -> {:?}",
                    prev.core_type,
                    config.core_type
                );
                config.core_type = prev.core_type;
            }
        }
    }

    // Desktop: core_type linkage to local core binary.
    #[cfg(not(target_os = "android"))]
    {
        let prev_core_type = ClientConfig::load(data_dir).ok().map(|c| c.core_type);
        if prev_core_type != Some(config.core_type) {
            let belongs = !config.core_binary.as_os_str().is_empty()
                && pp_client::infer_core_type(&config.core_binary) == Some(config.core_type);
            if !belongs {
                let inv = pp_client::ClientCoreInventory::new(data_dir.to_path_buf());
                match inv.preferred_binary(config.core_type) {
                    Some(path) => config.core_binary = path,
                    None => warnings.push(format!(
                        "核心类型已切换为 {}，但未找到该类型的本地核心，请到核心管理下载",
                        config.core_type
                    )),
                }
            }
        }
    }

    config.save().map_err(|e| format!("保存配置失败: {e}"))?;
    Ok(SaveConfigView {
        warning: if warnings.is_empty() {
            None
        } else {
            Some(warnings.join("；"))
        },
    })
}

/// Save config (`hub_url` / `sub_token` retired, no validation or warnings).
#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    cfg: ClientConfigView,
) -> Result<SaveConfigView, String> {
    save_config_impl(&state.data_dir, cfg)
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

    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_empty_path<T>(f: impl FnOnce() -> T) -> T {
        let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", "/nonexistent-pp-test-bin");
        }
        let result = f();
        match old {
            Some(v) => unsafe { std::env::set_var("PATH", v) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        result
    }

    fn full_view(data_dir: &std::path::Path) -> ClientConfigView {
        ClientConfigView {
            hub_url: "http://127.0.0.1:50052".to_string(),
            sub_token: "tok".to_string(),
            core_type: "singbox".to_string(),
            core_binary: data_dir
                .join("cores/sing-box/1.13.15/sing-box")
                .to_string_lossy()
                .into_owned(),
            ..ClientConfigView::default()
        }
    }

    fn write_core(data_dir: &std::path::Path, core_dir: &str, version: &str) {
        let bin = data_dir
            .join("cores")
            .join(core_dir)
            .join(version)
            .join(core_dir);
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"fake core").unwrap();
    }

    #[test]
    fn save_config_roundtrip_preserves_basic_settings_and_panel_features() {
        let dir = TestDir::new();
        let mut view = full_view(dir.path());
        view.mixed_port = 20000;
        view.mitm_enabled = false;
        view.system_proxy_enabled = true;
        view.tun_enabled = true;
        view.tun_stack = "system".to_string();
        view.tun_auto_route = false;
        view.clash_api_enabled = true;
        view.clash_api_port = 9091;
        view.clash_api_secret = "sekret".to_string();
        view.clash_api_ui = "yacd".to_string();
        view.github_proxy_prefix = "https://gh-proxy.com".to_string();
        view.fetch_via_local_proxy = true;
        view.rule_mode = "direct".to_string();

        let result = with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());
        assert!(result.warning.is_none(), "full payload should not warn: {:?}", result.warning);

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 20000);
        assert!(!saved.mitm_enabled);
        assert!(saved.system_proxy_enabled);
        assert!(saved.tun_enabled);
        assert_eq!(saved.tun_stack, "system");
        assert!(!saved.tun_auto_route);
        assert!(saved.clash_api_enabled);
        assert_eq!(saved.clash_api_port, 9091);
        assert_eq!(saved.clash_api_secret, "sekret");
        assert_eq!(saved.clash_api_ui, "yacd");
        assert_eq!(saved.github_proxy_prefix, "https://gh-proxy.com");
        assert!(saved.fetch_via_local_proxy);
        assert_eq!(saved.rule_mode, "direct");

        let view2 = ClientConfigView::from_config(&saved);
        assert_eq!(view2.mixed_port, 20000);
        assert!(!view2.mitm_enabled);
        assert!(view2.system_proxy_enabled);
        assert!(view2.tun_enabled);
        assert_eq!(view2.tun_stack, "system");
        assert!(view2.clash_api_enabled);
        assert_eq!(view2.clash_api_secret, "sekret");
        assert_eq!(view2.clash_api_ui, "yacd");
        assert_eq!(view2.github_proxy_prefix, "https://gh-proxy.com");
        assert!(view2.fetch_via_local_proxy);
        assert_eq!(view2.rule_mode, "direct");
    }

    #[test]
    fn save_config_partial_payload_defaults_and_saves() {
        let dir = TestDir::new();
        let json = serde_json::json!({
            "data_dir": "/tmp/pp",
            "hub_url": "",
            "sub_token": "",
            "core_type": "singbox",
            "core_binary": "",
            "mixed_port": 12345,
            "mitm_enabled": false,
            "mitm_hostnames": [],
            "mitm_script_dialect": "Surge",
        });
        let view: ClientConfigView = serde_json::from_value(json).unwrap();
        assert_eq!(view.clash_api_ui, "zashboard", "missing fields should default");
        assert!(view.github_proxy_prefix.is_empty() && !view.fetch_via_local_proxy,
            "old frontend missing GitHub fields should default");
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 12345);
        assert!(!saved.mitm_enabled);
        assert!(!saved.system_proxy_enabled, "missing bool fields should default");
        assert_eq!(saved.clash_api_ui, "zashboard");

        if let Some(w) = &result.warning {
            assert!(!w.contains("hub_url") && !w.contains("sub_token"),
                "empty hub_url/sub_token should not warn: {w}");
        }
    }

    #[test]
    fn save_config_empty_hub_and_token_saves_without_warning() {
        let dir = TestDir::new();
        let view = full_view(dir.path());

        with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());
        let mut cleared = view;
        cleared.hub_url = String::new();
        cleared.sub_token = String::new();
        cleared.mixed_port = 30000;
        let result = with_empty_path(|| save_config_impl(dir.path(), cleared).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 30000, "basic settings should save");
        assert!(result.warning.is_none(), "empty hub_url/sub_token should not warn: {:?}", result.warning);
    }

    #[test]
    fn save_config_switching_core_type_auto_fills_preferred_binary() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "mihomo", "1.19.29");
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let mut view = full_view(dir.path());
        view.core_type = "mihomo".to_string();
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::Mihomo);
        assert_eq!(saved.core_binary, dir.path().join("cores/mihomo/1.19.29/mihomo"));
        assert!(result.warning.is_none(), "found local core should not warn: {:?}", result.warning);
    }

    #[test]
    fn save_config_switching_core_type_without_local_core_keeps_and_warns() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let mut view = full_view(dir.path());
        view.core_type = "mihomo".to_string();
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::Mihomo);
        assert_eq!(saved.core_binary, dir.path().join("cores/sing-box/1.13.15/sing-box"));
        let warning = result.warning.expect("should return linkage warning");
        assert!(warning.contains("核心类型已切换"), "{warning}");
    }

    #[test]
    fn save_config_same_core_type_keeps_binary_untouched() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let view = full_view(dir.path());
        with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::SingBox);
        assert_eq!(saved.core_binary, dir.path().join("cores/sing-box/1.13.15/sing-box"));
    }
}
