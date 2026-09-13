//! Proxy lifecycle commands: start, stop, status, rule mode.

use pp_client::ClientConfig;
use serde::Serialize;
use tauri::State;

use crate::commands::TauriNotifier;
use crate::state::AppState;

/// External view of client runtime status.
#[derive(Debug, Clone, Serialize)]
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
    /// 本次启动未能本地化的内置 CN 规则集 tag（降级运行；空 = 完整分流）。
    pub missing_rule_sets: Vec<String>,
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
            missing_rule_sets: status.missing_rule_sets.clone(),
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
    // 降级启动（内置规则集未全部本地化）：后台重试下载，补齐后自动重载核心恢复完整
    // 分流（延迟下载能力；详见 pp_client::ruleset_manager 模块文档）。
    if !status.missing_rule_sets.is_empty() {
        spawn_ruleset_retry(std::sync::Arc::clone(&state.client), state.data_dir.clone());
    }
    Ok(ClientStatusView::from_status(&status))
}

/// 规则集后台重试任务：降级启动后每 20s 重试下载（上限 5 分钟），全部补齐且核心仍在
/// 运行时自动 stop+start 重载（重跑合成即获得完整 CN 分流）；核心被停止或超时即退出。
fn spawn_ruleset_retry(
    client: std::sync::Arc<tokio::sync::Mutex<Option<pp_client::ClientState>>>,
    data_dir: std::path::PathBuf,
) {
    tokio::spawn(async move {
        const MAX_ATTEMPTS: u32 = 15;
        const RETRY_INTERVAL: std::time::Duration = std::time::Duration::from_secs(20);
        for attempt in 1..=MAX_ATTEMPTS {
            tokio::time::sleep(RETRY_INTERVAL).await;
            // 读取 GitHub 代理前缀（可能已被用户修改）。
            let prefix = pp_client::ClientConfig::load(&data_dir)
                .map(|cfg| cfg.github_proxy_prefix)
                .unwrap_or_default();
            let available =
                pp_client::ruleset_manager::ensure_builtin_rule_sets(&data_dir, &prefix).await;
            if available.len() < pp_client::ruleset_manager::builtin_tags().len() {
                tracing::debug!(attempt, "规则集后台重试：仍未全部就绪");
                continue;
            }
            // 全部补齐：核心仍在运行则重载恢复完整分流。
            let mut lock = client.lock().await;
            let Some(state) = lock.as_mut() else {
                return;
            };
            if !state.status().await.core_running {
                return;
            }
            tracing::info!(attempt, "规则集后台重试补齐，自动重载核心恢复完整分流");
            state.stop().await;
            if let Err(e) = state.start().await {
                tracing::error!(error = %e, "规则集补齐后自动重载失败");
            }
            return;
        }
        tracing::warn!("规则集后台重试达到上限，保持降级运行（下次启动重试）");
    });
}

/// Stop proxy.
#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    let Some(client) = lock.as_mut() else {
        return Ok(idle_status_view(&state.data_dir));
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
        return Ok(idle_status_view(&state.data_dir));
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
        missing_rule_sets: Vec::new(),
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
    if status.core_running
        && client.config.clash_api_enabled
        && let Err(e) = pp_client::push_clash_mode(
            client.config.clash_api_port,
            &client.config.clash_api_secret,
            &mode,
        )
        .await
    {
        tracing::warn!(error = %e, mode = %mode, "Clash API hot-switch rule mode failed");
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
                "pp-client-ui-proxy-test-{}-{}",
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
            PathBuf::new(),
        );
        cfg.save().unwrap();

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.rule_mode, "rule");

        for mode in ["global", "direct", "rule"] {
            pp_client::set_rule_mode_persist(dir.path(), mode).unwrap();
            let saved = ClientConfig::load(dir.path()).unwrap();
            assert_eq!(
                saved.rule_mode, mode,
                "{mode} should persist to client.json"
            );
        }
    }

    #[test]
    fn idle_status_view_reports_persisted_rule_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
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

    /// Regression: `proxy_status` / `stop_proxy` return this view when no client
    /// instance exists (e.g. first launch before any `start_proxy`). It must
    /// report a normalized, non-empty rule mode so the home chip is not blank.
    #[test]
    fn idle_status_view_defaults_to_rule_without_saved_config() {
        let dir = TestDir::new();
        let view = idle_status_view(dir.path());
        assert_eq!(view.rule_mode, "rule");
        assert!(!view.rule_mode.is_empty());
    }

    /// The persisted `rule_mode` may be empty/legacy; the idle path must
    /// normalize it instead of surfacing the raw value.
    #[test]
    fn idle_status_view_normalizes_empty_persisted_rule_mode() {
        let dir = TestDir::new();
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            PathBuf::new(),
        );
        cfg.rule_mode = String::new();
        cfg.save().unwrap();

        let view = idle_status_view(dir.path());
        assert_eq!(view.rule_mode, "rule");
        assert!(!view.rule_mode.is_empty());
    }
}
