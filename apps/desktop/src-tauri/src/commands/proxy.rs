//! Proxy lifecycle commands: start, stop, status, rule mode.

use pp_client::ClientConfig;
use serde::Serialize;
use tauri::State;

use crate::commands::TauriNotifier;
use crate::state::AppState;

/// External view of client runtime status.
#[derive(Debug, Clone, Serialize, Default)]
pub struct ClientStatusView {
    pub core_running: bool,
    pub mitm_addr: Option<String>,
    pub system_proxy: bool,
    /// Current effective rule mode (`rule` / `global` / `direct`).
    pub rule_mode: String,
    /// Rule count of the current synthesized config (0 when not running).
    pub rule_count: u64,
    /// Clash dashboard API URL (when core running and clash_api_enabled).
    pub clash_api_url: Option<String>,
}

impl ClientStatusView {
    pub(crate) fn from_status(status: &pp_client::ClientStatus) -> Self {
        Self {
            core_running: status.core_running,
            mitm_addr: status.mitm_addr.map(|a| a.to_string()),
            system_proxy: status.system_proxy,
            rule_mode: status.rule_mode.clone(),
            rule_count: status.rule_count,
            clash_api_url: status.clash_api_url.clone(),
        }
    }
}

/// Start proxy (creates new ClientState from saved config if none exists).
#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    if lock.is_none() {
        let cfg = ClientConfig::load(&state.data_dir)
            .map_err(|e| format!("未找到已保存的配置（{e}），请先保存配置"))?;
        *lock = Some(pp_client::ClientState::with_notifier(
            cfg,
            std::sync::Arc::new(TauriNotifier::new(app)),
        ));
    }
    let client = lock
        .as_mut()
        .ok_or_else(|| "客户端状态初始化失败".to_string())?;
    client.start().await.map_err(|e| format!("启动失败: {e}"))?;
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// Stop proxy.
#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    let Some(client) = lock.as_mut() else {
        return Ok(ClientStatusView::default());
    };
    client.stop().await;
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// Query proxy runtime status.
#[tauri::command]
pub async fn proxy_status(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(ClientStatusView::default());
    };
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// Status view when no client instance is running (`rule_mode` from persisted config).
pub(crate) fn idle_status_view(data_dir: &std::path::Path) -> ClientStatusView {
    ClientStatusView {
        core_running: false,
        mitm_addr: None,
        system_proxy: false,
        rule_mode: ClientConfig::load(data_dir)
            .map(|c| c.normalized_rule_mode().to_string())
            .unwrap_or_else(|_| "rule".to_string()),
        rule_count: 0,
        clash_api_url: None,
    }
}

/// Set rule mode (`rule` / `global` / `direct`): persist to `client.json`.
///
/// Best-effort hot-switch via Clash API PATCH /configs when applicable.
#[tauri::command]
pub async fn set_rule_mode(
    state: State<'_, AppState>,
    mode: String,
) -> Result<ClientStatusView, String> {
    pp_client::set_rule_mode_persist(&state.data_dir, &mode)
        .map_err(|e| format!("规则模式设置失败: {e}"))?;
    let mut lock = state.client.lock().await;
    let Some(client) = lock.as_mut() else {
        return Ok(idle_status_view(&state.data_dir));
    };
    client.config.rule_mode = mode.clone();
    let status = client.status().await;
    if status.core_running && client.config.clash_api_enabled {
        if let Err(e) = pp_client::push_clash_mode(
            client.config.clash_api_port,
            &client.config.clash_api_secret,
            &mode,
        )
        .await
        {
            tracing::warn!(error = %e, mode = %mode, "Clash API hot-switch rule mode failed");
        }
    }
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

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

    #[test]
    fn set_rule_mode_rejects_invalid_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            pp_common::CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        for invalid in ["", "bogus", "Rule", "全局"] {
            let err = pp_client::set_rule_mode_persist(dir.path(), invalid).unwrap_err();
            assert!(err.contains("Invalid rule mode"), "{invalid:?}: {err}");
        }
    }

    #[test]
    fn set_rule_mode_persists_valid_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            pp_common::CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.rule_mode, "rule");

        for mode in ["global", "direct", "rule"] {
            pp_client::set_rule_mode_persist(dir.path(), mode).unwrap();
            let saved = ClientConfig::load(dir.path()).unwrap();
            assert_eq!(saved.rule_mode, mode, "{mode} should persist to client.json");
        }
    }

    #[test]
    fn idle_status_view_reports_persisted_rule_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            pp_common::CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();
        let view = idle_status_view(dir.path());
        assert_eq!(view.rule_mode, "rule");
        assert_eq!(view.rule_count, 0);
        assert!(!view.core_running);
        assert!(view.clash_api_url.is_none());

        pp_client::set_rule_mode_persist(dir.path(), "direct").unwrap();
        let view = idle_status_view(dir.path());
        assert_eq!(view.rule_mode, "direct");
    }
}
