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
use std::time::Duration;

use pp_common::{CoreType, PanelResult};
use pp_mitm::{MemoryRecorder, RewriteEngine, RunningProxy, ScriptHookEngine};
use pp_script::{MemoryPersistentStore, ScriptHost, ScriptLimits, ScriptScheduler, TaskScript};
use tokio::task::JoinHandle;

use crate::config::ClientConfig;
use crate::core_config;
use crate::http_exec::ReqwestHttpExecutor;
use crate::mitm::{MitmBuildOptions, build_mitm_proxy};
use crate::remote::{RemoteManager, TracingNotifier};
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

/// 定时任务调度器运行句柄。
struct SchedulerHandle {
    scheduler: Arc<ScriptScheduler>,
    handle: JoinHandle<()>,
}

/// 客户端运行状态编排器。
pub struct ClientState {
    /// 客户端配置。
    pub config: ClientConfig,
    core: Option<CoreRunner>,
    mitm: Option<RunningProxy>,
    sysproxy: Arc<dyn SystemProxy>,
    /// 抓包记录器（内存环形缓冲，容量 2048；随 MITM 代理启动注入）。
    recorder: Arc<MemoryRecorder>,
    /// 远程订阅 task 脚本调度器（MITM 就绪后启动，stop 时停止）。
    scheduler: Option<SchedulerHandle>,
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
            recorder: Arc::new(MemoryRecorder::new(2048)),
            scheduler: None,
        }
    }

    /// 抓包记录器（内存环形缓冲，容量 2048）。
    pub fn recorder(&self) -> Arc<MemoryRecorder> {
        Arc::clone(&self.recorder)
    }

    /// 定时任务调度器（远程订阅 task 脚本）；未启用 MITM 或没有任务时为 `None`。
    pub fn scheduler(&self) -> Option<&ScriptScheduler> {
        self.scheduler.as_ref().map(|h| h.scheduler.as_ref())
    }

    /// 定时任务调度器的 `Arc` 句柄（供外部在独立线程驱动脚本执行；未就绪时为 `None`）。
    pub fn scheduler_handle(&self) -> Option<Arc<ScriptScheduler>> {
        self.scheduler.as_ref().map(|h| Arc::clone(&h.scheduler))
    }

    /// 启动：订阅 → 合成配置 → 核心 → MITM → 系统代理，失败回滚。
    pub async fn start(&mut self) -> PanelResult<()> {
        tracing::info!(hub_url = %self.config.hub_url, "客户端启动：拉取订阅");
        let fetcher = subscription::SubscriptionFetcher::new();
        let config_json = match self.config.core_type {
            CoreType::SingBox => {
                let (sub_config, _info) = fetcher
                    .fetch_singbox_config(&self.config.hub_url, &self.config.sub_token)
                    .await?;
                core_config::compose_singbox_config(&sub_config, self.config.mixed_port)?
            }
            CoreType::Mihomo => {
                let (yaml, _info) = fetcher
                    .fetch_clash_config(&self.config.hub_url, &self.config.sub_token)
                    .await?;
                core_config::compose_mihomo_config(&yaml, self.config.mixed_port)?
            }
        };
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
            // 读取远程订阅缓存：重写规则 / 脚本钩子 / 主机名 / 定时任务。
            let remote = RemoteManager::new(self.config.data_dir.clone());
            let merged = match remote.load_cached() {
                Ok(m) => m,
                Err(e) => {
                    self.stop_core().await;
                    return Err(e);
                }
            };
            let rewrite = RewriteEngine {
                rules: merged.rewrites,
            };
            let host = Arc::new(ScriptHost::new(
                Arc::new(ReqwestHttpExecutor::new()),
                Arc::new(MemoryPersistentStore::new()),
                Arc::new(TracingNotifier::new()),
            ));
            let hooks = ScriptHookEngine::new(
                Arc::clone(&host),
                self.config.mitm.script_dialect,
                ScriptLimits::default(),
                merged.scripts,
            );
            let options = MitmBuildOptions {
                extra_hostnames: merged.hostnames,
                rewrite,
                hooks: Some(hooks),
            };
            let proxy = match build_mitm_proxy(&self.config, options, self.recorder.clone()) {
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
            // 定时任务调度器（MITM 就绪后启动；失败回滚 MITM 与核心）。
            if let Err(e) = self.start_scheduler(host, merged.task_scripts).await {
                self.stop_mitm().await;
                self.stop_core().await;
                return Err(e);
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
        self.stop_scheduler().await;
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

    /// 启动远程订阅 task 脚本调度器；cron 表达式非法的任务跳过并记警告。
    async fn start_scheduler(
        &mut self,
        host: Arc<ScriptHost>,
        tasks: Vec<TaskScript>,
    ) -> PanelResult<()> {
        let mut scheduler = ScriptScheduler::new(host, ScriptLimits::default());
        let mut registered = 0usize;
        for task in tasks {
            if !task.enabled {
                continue;
            }
            match scheduler.add_task(task) {
                Ok(()) => registered += 1,
                Err(e) => {
                    tracing::warn!(error = %e, "skip scheduled task with invalid cron expression")
                }
            }
        }
        if registered == 0 {
            return Ok(());
        }
        let scheduler = Arc::new(scheduler);
        let handle = Arc::clone(&scheduler).start().await?;
        self.scheduler = Some(SchedulerHandle { scheduler, handle });
        Ok(())
    }

    /// 停止调度器：发送停止信号并等待后台循环退出。
    async fn stop_scheduler(&mut self) {
        if let Some(handle) = self.scheduler.take() {
            handle.scheduler.stop().await;
            let _ = tokio::time::timeout(Duration::from_secs(5), handle.handle).await;
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
    use crate::remote::{RemoteKind, RemoteResource};
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

    /// 完整集成 server（禁外部网络）：
    /// `/sub/{token}` 订阅配置、`/snippet` 远程 QX 片段（引用同 server 的脚本 URL）。
    async fn spawn_integration_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let sub_body = r#"{
            "log": {"level": "info"},
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 1}],
            "outbounds": [{"type": "direct"}]
        }"#;
        let snippet = format!(
            "[rewrite_local]\n\
             ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
             ^https?://example\\.com/rsp script-response-body {base}/hook.js\n\
             \n\
             [task_local]\n\
             0 9 * * * {base}/task.js, tag=每日签到\n\
             \n\
             [mitm]\n\
             hostname = *.example.com, api.example2.com\n"
        );
        let app = axum::Router::new()
            .route(
                "/sub/{token}",
                axum::routing::get(move || async move { sub_body }),
            )
            .route(
                "/snippet",
                axum::routing::get(move || async move { snippet }),
            )
            .route(
                "/hook.js",
                axum::routing::get(|| async { "const hook = 1;" }),
            )
            .route(
                "/task.js",
                axum::routing::get(|| async { "const task = 2; $done({});" }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        base
    }

    /// 远程 snippet 缓存 → MITM（rewrite/hooks/hostnames 注入）+ cron 调度器（task 脚本）。
    #[tokio::test]
    async fn start_with_remote_snippet_runs_mitm_and_scheduler() {
        let base = spawn_integration_server().await;
        let dir = tempfile::tempdir().unwrap();

        // 预置 remotes.json（指向本地 snippet server）并先拉取一次写缓存。
        let remote = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "rules".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: pp_script::ScriptDialect::QuantumultX,
            ..RemoteResource::default()
        }];
        remote.save(&remotes).unwrap();
        let report = remote.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1, "snippet fetch should succeed");

        let mut cfg = test_config(&dir, base);
        cfg.mitm_enabled = true;
        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        // MITM 已运行，且调度器持有远程 snippet 的 task 脚本。
        let status = state.status().await;
        assert!(status.mitm_addr.is_some(), "MITM 应已运行");
        let tasks = state
            .scheduler()
            .expect("scheduler should run")
            .list_tasks();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "每日签到");

        // stop 正常：逆序关闭系统代理 → MITM → 调度器 → 核心。
        state.stop().await;
        let status = state.status().await;
        assert!(status.mitm_addr.is_none());
        assert!(!status.core_running);
    }

    /// core_type=Mihomo：走 clash 订阅拉取 + mihomo 配置合成，假核心启动成功。
    #[tokio::test]
    async fn start_with_mihomo_core_fetches_clash_and_starts() {
        let yaml =
            "port: 7890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
        let addr = spawn_server(StatusCode::OK, yaml).await;
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = test_config(&dir, format!("http://{addr}"));
        cfg.core_type = CoreType::Mihomo;

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        let status = state.status().await;
        assert!(status.core_running, "mihomo 核心应启动");

        state.stop().await;
        let status = state.status().await;
        assert!(!status.core_running);
    }
}
