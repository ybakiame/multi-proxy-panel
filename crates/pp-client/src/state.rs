//! 客户端运行状态编排。
//!
//! [`ClientState`] 编排客户端整体生命周期：
//!
//! **启动**：拉取订阅 → 合成核心配置 → 启动核心 → （可选）启动 MITM →
//! （可选）启用系统代理。任一步失败时回滚已完成步骤。
//!
//! **停止**：按启动逆序逐项关闭（best-effort，单项失败不影响其余）。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use pp_common::PanelResult;
use pp_mitm::RunningProxy;

use crate::config::ClientConfig;
use crate::core_config;
use crate::mitm::build_mitm_proxy;
use crate::runner::CoreRunner;
use crate::subscription;
use crate::sysproxy::{PlatformSystemProxy, SystemProxy};

/// 客户端当前运行状态。
#[derive(Debug, Clone)]
pub struct ClientStatus {
    /// 核心是否在运行。
    pub core_running: bool,
    /// MITM 代理监听地址（未启用时为 `None`）。
    pub mitm_addr: Option<SocketAddr>,
    /// 系统代理当前是否启用。
    pub system_proxy: bool,
}

/// 客户端运行状态编排器。
pub struct ClientState {
    /// 客户端配置。
    pub config: ClientConfig,
    core: Option<CoreRunner>,
    mitm: Option<RunningProxy>,
    sysproxy: Arc<dyn SystemProxy>,
}

impl ClientState {
    /// 使用平台系统代理实现创建状态机。
    pub fn new(config: ClientConfig) -> Self {
        Self::with_system_proxy(config, Arc::new(PlatformSystemProxy::default()))
    }

    /// 使用自定义系统代理实现创建状态机（测试注入用）。
    pub fn with_system_proxy(config: ClientConfig, sysproxy: Arc<dyn SystemProxy>) -> Self {
        Self {
            config,
            core: None,
            mitm: None,
            sysproxy,
        }
    }

    /// 启动：订阅 → 合成配置 → 核心 → MITM → 系统代理，失败回滚。
    pub async fn start(&mut self) -> PanelResult<()> {
        tracing::info!(hub_url = %self.config.hub_url, "客户端启动：拉取订阅");
        let fetcher = subscription::SubscriptionFetcher::new();
        let (sub_config, _info) = fetcher
            .fetch_singbox_config(&self.config.hub_url, &self.config.sub_token)
            .await?;
        let config_json = core_config::compose_singbox_config(&sub_config, self.config.mixed_port)?;
        self.start_services(&config_json).await
    }

    /// 在订阅与配置合成之后，启动核心 →（可选）MITM →（可选）系统代理，失败回滚。
    ///
    /// 回滚策略：核心启动成功后，若 MITM 构建/启动失败则停止核心；若系统代理启用
    /// 失败则按逆序关闭 MITM 与核心，最后把错误向上传播。
    async fn start_services(&mut self, config_json: &serde_json::Value) -> PanelResult<()> {
        tracing::info!(binary = %self.config.core_binary.display(), "启动核心");
        let core = CoreRunner::create(
            self.config.core_type,
            &self.config.core_binary,
            &self.config.data_dir,
        )?;
        core.start(config_json).await?;
        self.core = Some(core);

        if self.config.mitm_enabled {
            tracing::info!("启动 MITM 代理");
            let proxy = match build_mitm_proxy(&self.config, None) {
                Ok(p) => p,
                Err(e) => {
                    self.stop_core().await;
                    return Err(e);
                }
            };
            match proxy.start().await {
                Ok(running) => self.mitm = Some(running),
                Err(e) => {
                    self.stop_core().await;
                    return Err(e);
                }
            }
        }

        if self.config.system_proxy_enabled {
            let addr = self.mitm.as_ref().map(|m| m.addr).unwrap_or_else(|| {
                SocketAddr::new(Ipv4Addr::LOCALHOST.into(), self.config.mixed_port)
            });
            if let Err(e) = self.sysproxy.enable(addr).await {
                tracing::error!(%addr, "启用系统代理失败，回滚 MITM 与核心");
                self.stop_mitm().await;
                self.stop_core().await;
                return Err(e);
            }
        }

        Ok(())
    }

    /// 停止：按启动逆序逐项关闭（best-effort）。
    pub async fn stop(&mut self) {
        let _ = self.sysproxy.disable().await;
        self.stop_mitm().await;
        self.stop_core().await;
    }

    /// 当前运行状态。
    pub async fn status(&self) -> ClientStatus {
        let core_running = match &self.core {
            Some(c) => c.is_running().await,
            None => false,
        };
        ClientStatus {
            core_running,
            mitm_addr: self.mitm.as_ref().map(|m| m.addr),
            system_proxy: self.sysproxy.is_enabled().await,
        }
    }

    async fn stop_core(&mut self) {
        if let Some(core) = self.core.take() {
            let _ = core.stop().await;
        }
    }

    async fn stop_mitm(&mut self) {
        if let Some(m) = self.mitm.take() {
            m.shutdown();
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::Arc;

    use axum::http::StatusCode;
    use pp_common::CoreType;
    use tempfile::TempDir;

    use crate::config::ClientConfig;
    use crate::sysproxy::MockSystemProxy;

    /// 启动一个本地 axum 服务，对所有路径返回 `(status, body)`（测试用，无外部请求）。
    async fn spawn_server(status: StatusCode, body: &'static str) -> SocketAddr {
        let app = axum::Router::new().fallback(move || async move { (status, body) });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        addr
    }

    /// 写一个「忽略参数、持续运行」的假核心脚本。
    fn fake_core_script(dir: &TempDir) -> PathBuf {
        let path = dir.path().join("fake-core.sh");
        std::fs::write(&path, "#!/bin/sh\nsleep 5\n").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn test_config(dir: &TempDir, hub_url: String) -> ClientConfig {
        ClientConfig::new(
            dir.path().to_path_buf(),
            hub_url,
            "tok",
            CoreType::SingBox,
            fake_core_script(dir),
        )
    }

    /// 假核心脚本必须能被 CoreManager 真实拉起/停止，否则回滚测试不具代表性。
    #[tokio::test]
    async fn fake_core_binary_starts_and_stops() {
        let dir = tempfile::tempdir().unwrap();
        let core_bin = fake_core_script(&dir);
        let runner = CoreRunner::create(CoreType::SingBox, &core_bin, dir.path()).unwrap();
        let config = serde_json::json!({"log": {"level": "info"}});
        runner.start(&config).await.unwrap();
        assert!(runner.is_running().await);
        runner.stop().await.unwrap();
        assert!(!runner.is_running().await);
    }

    /// 订阅失败 → 不启动核心 / MITM，且不启用系统代理。
    #[tokio::test]
    async fn start_rolls_back_on_subscription_failure() {
        let addr = spawn_server(StatusCode::INTERNAL_SERVER_ERROR, "oops").await;
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = test_config(&dir, format!("http://{addr}"));
        cfg.system_proxy_enabled = true;

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        assert!(state.start().await.is_err());

        // 订阅失败：系统代理从未启用。
        assert_eq!(mock.calls(), vec![]);
        let status = state.status().await;
        assert!(!status.core_running);
        assert!(status.mitm_addr.is_none());
    }

    /// MITM 构建失败（CA 目录被文件占位）→ 回滚核心，且不启用系统代理。
    #[tokio::test]
    async fn start_rolls_back_when_mitm_build_fails() {
        let body = r#"{
            "log": {"level": "info"},
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 1}],
            "outbounds": [{"type": "direct"}]
        }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = test_config(&dir, format!("http://{addr}"));
        cfg.system_proxy_enabled = true;
        cfg.mitm_enabled = true;
        // ca_dir 被普通文件占位 → FileCaStore 无法写 CA → build_mitm_proxy 失败。
        std::fs::write(dir.path().join("certs"), b"i am a file").unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        assert!(state.start().await.is_err());

        // MITM 构建失败：系统代理不启用，且核心已被回滚停止。
        assert_eq!(mock.enable_count(), 0);
        let status = state.status().await;
        assert!(status.mitm_addr.is_none());
        assert!(!status.core_running);
    }
}
