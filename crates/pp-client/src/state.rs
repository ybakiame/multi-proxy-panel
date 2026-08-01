//! 客户端运行状态编排。
//!
//! [`ClientState`] 编排客户端整体生命周期：
//!
//! **启动**：拉取订阅 → （可选）启动 MITM（拿到监听地址）→ 合成核心配置
//! （注入 MITM 路由规则与回流入口）→ 启动核心 → 启用系统代理（指向核心
//! mixed 主入口）。任一步失败时回滚已完成步骤。
//!
//! **停止**：按启动逆序逐项关闭（best-effort，单项失败不影响其余）。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use pp_common::{CoreType, PanelError, PanelResult};
use pp_mitm::{MemoryRecorder, RewriteEngine, RunningProxy, ScriptHookEngine};
use pp_script::{
    FilePersistentStore, Notifier, ScriptHost, ScriptLimits, ScriptScheduler, TaskScript,
};
use tokio::task::JoinHandle;

use crate::config::ClientConfig;
use crate::core_config;
use crate::http_exec::ReqwestHttpExecutor;
use crate::mitm::{MitmBuildOptions, build_mitm_proxy};
use crate::profile;
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
    /// 脚本通知器（`$notify` / `$notification`）；默认 [`TracingNotifier`]（仅日志）。
    notifier: Arc<dyn Notifier>,
    /// 抓包记录器（内存环形缓冲，容量 2048；随 MITM 代理启动注入）。
    recorder: Arc<MemoryRecorder>,
    /// 远程订阅 task 脚本调度器（MITM 就绪后启动，stop 时停止）。
    scheduler: Option<SchedulerHandle>,
}

impl ClientState {
    /// 使用平台系统代理实现创建状态机。
    pub fn new(config: ClientConfig) -> Self {
        Self::with_dependencies(
            config,
            Arc::new(PlatformSystemProxy::default()),
            Arc::new(TracingNotifier::new()),
        )
    }

    /// 使用自定义系统代理实现创建状态机（测试注入用）。
    pub fn with_system_proxy(config: ClientConfig, sysproxy: Arc<dyn SystemProxy>) -> Self {
        Self::with_dependencies(config, sysproxy, Arc::new(TracingNotifier::new()))
    }

    /// 使用自定义通知器创建状态机（OS 桌面通知接入用）。
    pub fn with_notifier(config: ClientConfig, notifier: Arc<dyn Notifier>) -> Self {
        Self::with_dependencies(config, Arc::new(PlatformSystemProxy::default()), notifier)
    }

    /// 组合依赖构造状态机。
    fn with_dependencies(
        config: ClientConfig,
        sysproxy: Arc<dyn SystemProxy>,
        notifier: Arc<dyn Notifier>,
    ) -> Self {
        Self {
            config,
            core: None,
            mitm: None,
            sysproxy,
            notifier,
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

    /// 启动：订阅 → Profile（模板 + 双复写）→ MITM → 合成配置（MITM 链路）→ 核心 →
    /// 系统代理，失败回滚。
    ///
    /// 启动顺序：先拉取订阅并经 Profile 层生成基础配置（订阅只取节点 → 本地模板 →
    /// YAML/JS 双复写），MITM 启用时再启动 MITM（拿到监听地址），合成配置时注入
    /// [`MitmChain`]（路由规则 + 回流入口），随后启动核心，最后启用系统代理
    /// （始终指向核心 mixed 主入口，MITM 挂在核心之后由核心路由规则分发）。Profile
    /// 构建失败时尚未启动任何组件，无需回滚。
    pub async fn start(&mut self) -> PanelResult<()> {
        tracing::info!("客户端启动：解析订阅");
        let sub_store = subscription::SubscriptionStore::new(self.config.data_dir.clone());

        // 订阅 → Profile 层：订阅内容只用于提取节点，本地模板生成分组/路由，
        // YAML 与 JS 双复写（取 data_dir/profiles.json 中当前核心类型启用的模板）。
        //
        // 新路径：SubscriptionStore 取第一个 enabled 订阅，经 fetch_subscription
        // 嗅探格式并转成双核心节点。subscriptions.json 为空且旧 hub_url/sub_token
        // 非空时回退旧版 Hub 订阅路径（deprecated）。
        let sub_content = if let Some(sub) = sub_store.load()?.into_iter().find(|s| s.enabled) {
            let fetch = subscription::fetch_subscription(&sub.url).await?;
            match self.config.core_type {
                CoreType::SingBox => profile::SubContent::SingBox(serde_json::json!({
                    "outbounds": fetch.singbox_nodes,
                })),
                CoreType::Mihomo => {
                    let yaml = serde_yaml::to_string(&serde_json::json!({
                        "proxies": fetch.mihomo_nodes,
                    }))?;
                    profile::SubContent::Mihomo(yaml)
                }
            }
        } else if !self.config.hub_url.is_empty() && !self.config.sub_token.is_empty() {
            tracing::warn!(
                hub_url = %self.config.hub_url,
                "未配置通用订阅，回退到旧版 Hub 订阅路径（deprecated）"
            );
            let fetcher = subscription::SubscriptionFetcher::new();
            match self.config.core_type {
                CoreType::SingBox => {
                    let (sub_config, _info) = fetcher
                        .fetch_singbox_config(&self.config.hub_url, &self.config.sub_token)
                        .await?;
                    profile::SubContent::SingBox(sub_config)
                }
                CoreType::Mihomo => {
                    let (yaml, _info) = fetcher
                        .fetch_clash_config(&self.config.hub_url, &self.config.sub_token)
                        .await?;
                    profile::SubContent::Mihomo(yaml)
                }
            }
        } else {
            return Err(PanelError::Client(
                "no enabled subscription and no legacy hub subscription configured".to_string(),
            ));
        };
        let store = profile::ProfileStoreV2::new(self.config.data_dir.clone());
        let overrides = store.active_for(self.config.core_type)?.unwrap_or_default();
        let profile_cfg =
            profile::build_core_config(self.config.core_type, &sub_content, &overrides).await?;

        // MITM 先于核心启动：拿到监听地址才能注入核心路由规则。
        let chain = self.start_mitm_chain().await?;
        let config_json = match self.config.core_type {
            CoreType::SingBox => {
                let cfg = core_config::compose_singbox_config(
                    &profile_cfg,
                    self.config.mixed_port,
                    chain,
                );
                match cfg {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        self.rollback_mitm_started().await;
                        return Err(e);
                    }
                }
            }
            CoreType::Mihomo => {
                let yaml = serde_yaml::to_string(&profile_cfg)?;
                let cfg = core_config::compose_mihomo_config(&yaml, self.config.mixed_port, chain);
                match cfg {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        self.rollback_mitm_started().await;
                        return Err(e);
                    }
                }
            }
        };
        self.start_services(&config_json).await
    }

    /// MITM 启用时启动 MITM 并返回链路信息；未启用时返回 `None`。
    ///
    /// 读取远程订阅缓存（重写规则 / 脚本钩子 / 主机名 / 定时任务），构建并启动
    /// MITM，上游指向核心回流 mixed 入口（`mixed_port + 1`）。调度器启动失败时
    /// 回滚已启动的 MITM；其余失败发生在 MITM 启动之前，无需回滚。
    async fn start_mitm_chain(&mut self) -> PanelResult<Option<core_config::MitmChain>> {
        if !self.config.mitm_enabled {
            return Ok(None);
        }
        tracing::info!("启动 MITM 代理");
        // 读取远程订阅缓存：重写规则 / 脚本钩子 / 主机名 / 定时任务。
        let remote = RemoteManager::new(self.config.data_dir.clone());
        let merged = match remote.load_cached() {
            Ok(m) => m,
            Err(e) => return Err(e),
        };
        // 合并白名单（本地配置 + 远程订阅），供 MITM 与核心路由规则共用。
        let mut hostnames = self.config.mitm.hostnames.clone();
        for extra in &merged.hostnames {
            if !hostnames.contains(extra) {
                hostnames.push(extra.clone());
            }
        }
        let rewrite = RewriteEngine {
            rules: merged.rewrites,
        };
        let host = Arc::new(ScriptHost::new(
            Arc::new(ReqwestHttpExecutor::new()),
            Arc::new(FilePersistentStore::new(
                self.config.data_dir.join("script_store"),
            )),
            Arc::clone(&self.notifier),
        ));
        let hooks = ScriptHookEngine::new(
            Arc::clone(&host),
            self.config.mitm.script_dialect,
            ScriptLimits::default(),
            merged.scripts,
        );
        // MITM 上游指向核心回流 mixed 入口（mixed_port + 1）。
        let return_port = self.config.mixed_port + 1;
        let options = MitmBuildOptions {
            extra_hostnames: hostnames.clone(),
            upstream_port: Some(return_port),
            rewrite,
            hooks: Some(hooks),
        };
        let proxy = match build_mitm_proxy(&self.config, options, self.recorder.clone()) {
            Ok(p) => p,
            Err(e) => return Err(e),
        };
        let running = match proxy.start().await {
            Ok(r) => r,
            Err(e) => return Err(e),
        };
        let mitm_addr = running.addr;
        self.mitm = Some(running);
        // 定时任务调度器（MITM 就绪后启动；失败回滚 MITM）。
        if let Err(e) = self.start_scheduler(host, merged.task_scripts).await {
            self.stop_mitm().await;
            return Err(e);
        }
        Ok(Some(core_config::MitmChain {
            proxy_addr: mitm_addr,
            return_port,
            hostnames,
        }))
    }

    /// MITM 已启动后、后续步骤失败时的回滚：关闭 MITM 与调度器。
    async fn rollback_mitm_started(&mut self) {
        self.stop_mitm().await;
        self.stop_scheduler().await;
    }

    /// 在订阅与配置合成之后，启动核心 → 启用系统代理（指向核心 mixed 主入口），
    /// 失败回滚。MITM 已在 [`Self::start`] 中先于核心启动。
    ///
    /// 回滚策略：核心启动失败则关闭 MITM 与调度器；系统代理启用失败则按逆序
    /// 关闭核心、MITM 与调度器，最后把错误向上传播。
    async fn start_services(&mut self, config_json: &serde_json::Value) -> PanelResult<()> {
        tracing::info!(binary = %self.config.core_binary.display(), "启动核心");
        let core = CoreRunner::create(
            self.config.core_type,
            &self.config.core_binary,
            &self.config.data_dir,
        )?;
        if let Err(e) = core.start(config_json).await {
            self.rollback_mitm_started().await;
            return Err(e);
        }
        self.core = Some(core);

        if self.config.system_proxy_enabled {
            // 系统代理始终指向核心 mixed 主入口（MITM 挂在核心之后）。
            let addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), self.config.mixed_port);
            if let Err(e) = self.sysproxy.enable(addr).await {
                tracing::error!(%addr, "启用系统代理失败，回滚核心与 MITM");
                self.stop_core().await;
                self.rollback_mitm_started().await;
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
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use axum::http::StatusCode;
    use base64::Engine as _;
    use pp_common::CoreType;
    use tempfile::TempDir;

    use crate::config::ClientConfig;
    use crate::remote::{RemoteKind, RemoteResource};
    use crate::sysproxy::{MockSystemProxy, SysProxyCall};

    /// 记录通知的 Notifier（验证注入链路：ClientState → ScriptHost → `$notify`）。
    #[derive(Debug, Default)]
    struct RecordingNotifier {
        calls: std::sync::Mutex<Vec<(String, String, String)>>,
    }

    impl RecordingNotifier {
        fn new() -> Self {
            Self::default()
        }

        fn calls(&self) -> Vec<(String, String, String)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Notifier for RecordingNotifier {
        fn notify(
            &self,
            title: &str,
            subtitle: &str,
            body: &str,
            _options: Option<serde_json::Value>,
        ) {
            self.calls.lock().unwrap().push((
                title.to_string(),
                subtitle.to_string(),
                body.to_string(),
            ));
        }
    }

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

    /// 写一个把 `-c <config>` 参数对应的配置文件复制到 `capture` 的假核心脚本
    /// （用于断言核心实际收到的合成配置，从而验证 MITM 先于核心启动）。
    fn fake_core_capturing_args(dir: &TempDir, capture: &Path) -> PathBuf {
        let path = dir.path().join("fake-core-capture.sh");
        let script = format!(
            "#!/bin/sh\n\
             prev=\"\"\n\
             for arg in \"$@\"; do\n\
               if [ \"$prev\" = \"-c\" ]; then\n\
                 cp \"$arg\" {}\n\
               fi\n\
               prev=\"$arg\"\n\
             done\n\
             sleep 5\n",
            capture.display()
        );
        std::fs::write(&path, script).unwrap();
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
        // 订阅内容现在只取节点：含 2 个叶子节点 + selector / direct（提取时被过滤）。
        let sub_body = r#"{
            "log": {"level": "debug"},
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 1}],
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } },
                { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
                  "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
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
                axum::routing::get(|| async {
                    "const task = 2; $notify(\"签到成功\", \"test\", \"hello\"); $done({code: 0});"
                }),
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

    /// 通用订阅集成：subscriptions.json 指向本地 server（base64 分享链接）→ start 成功。
    #[tokio::test]
    async fn start_with_subscription_store_fetches_and_starts() {
        // 本地 server 返回 base64 编码的 vless 分享链接（禁外部网络）。
        let link = "vless://12345678-1234-1234-1234-123456789012@example.com:443?security=tls&sni=example.com#n1";
        let body = base64::engine::general_purpose::STANDARD.encode(link);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        tokio::spawn(async move {
            let app = axum::Router::new().route(
                "/sub",
                axum::routing::get(move || async move {
                    (
                        [(
                            "subscription-userinfo",
                            "upload=1; download=2; total=100; expire=3",
                        )],
                        body,
                    )
                }),
            );
            axum::serve(listener, app).await.unwrap();
        });

        let dir = tempfile::tempdir().unwrap();
        // subscriptions.json 指向本地 server。
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        store.add("local", &format!("{base}/sub"), true).unwrap();

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.mitm_enabled = false;

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        let status = state.status().await;
        assert!(status.core_running, "通用订阅路径核心应启动");

        state.stop().await;
        let status = state.status().await;
        assert!(!status.core_running);
    }

    #[tokio::test]
    async fn start_with_mitm_chain_runs_mitm_before_core_and_proxy_points_at_main_port() {
        let base = spawn_integration_server().await;
        let dir = tempfile::tempdir().unwrap();

        // 预置远程 snippet 缓存（含 MITM 白名单 *.example.com / api.example2.com）。
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

        // 假核心把收到的合成配置复制出来，供断言核心实际配置。
        let capture = dir.path().join("core-config-capture.json");
        let core_bin = fake_core_capturing_args(&dir, &capture);
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            base,
            "tok",
            CoreType::SingBox,
            core_bin,
        );
        cfg.mitm_enabled = true;
        cfg.system_proxy_enabled = true;

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        let status = state.status().await;
        let mitm_addr = status.mitm_addr.expect("MITM 应已运行");

        // 系统代理指向核心 mixed 主端口（而非 MITM 随机端口）。
        let calls = mock.calls();
        assert_eq!(calls.len(), 1, "系统代理应只启用一次");
        match &calls[0] {
            SysProxyCall::Enable(addr) => {
                assert_eq!(addr.port(), 17890, "系统代理应指向核心 mixed 主入口端口")
            }
            SysProxyCall::Disable => panic!("不应出现 disable"),
        }

        // MITM 先于核心：核心收到的配置里 pp-mitm outbound 端口 == MITM 实际
        // 监听端口（随机端口，只有 MITM 启动后才能注入配置）。
        let mut attempts = 0;
        while !capture.exists() && attempts < 100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            attempts += 1;
        }
        assert!(capture.exists(), "假核心应已复制合成配置");
        let core_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&capture).unwrap()).unwrap();

        // 双 mixed 入站：主入口 + 回流入口。
        let inbounds = core_config["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2);
        assert_eq!(inbounds[0]["tag"], "main-in");
        assert_eq!(inbounds[0]["listen_port"], 17890);
        assert_eq!(inbounds[1]["tag"], "mitm-return");
        assert_eq!(inbounds[1]["listen_port"], 17891);

        // pp-mitm outbound 指向 MITM 实际监听端口。
        let outbounds = core_config["outbounds"].as_array().unwrap();
        let pp_mitm = outbounds
            .iter()
            .find(|o| o["tag"] == "pp-mitm")
            .expect("核心配置应含 pp-mitm outbound");
        assert_eq!(pp_mitm["type"], "http");
        assert_eq!(pp_mitm["server_port"], mitm_addr.port());

        // 白名单路由规则：inbound 匹配主入口，域名按通配/精确正确分流。
        let rules = core_config["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0]["inbound"], serde_json::json!(["main-in"]));
        assert_eq!(
            rules[0]["domain_suffix"],
            serde_json::json!(["example.com"])
        );
        assert_eq!(rules[0]["domain"], serde_json::json!(["api.example2.com"]));
        assert_eq!(rules[0]["outbound"], "pp-mitm");

        state.stop().await;
        let status = state.status().await;
        assert!(status.mitm_addr.is_none());
        assert!(!status.core_running);
    }

    /// Profile 层集成：订阅（2 节点 + selector/direct 组）经模板分组、profiles.json
    /// 中启用模板的 JS 复写生效，compose 正常注入 inbounds 与 MITM 链。
    #[tokio::test]
    async fn start_with_profile_applies_template_groups_and_js_override() {
        let body = r#"{
            "log": {"level": "debug"},
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } },
                { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
                  "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
        }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();

        // 预置 profiles.json：启用一个 SingBox 模板，其 js_override 修改 dns.strategy。
        profile::ProfileStoreV2::new(dir.path().to_path_buf())
            .save(&[profile::Profile {
                id: uuid::Uuid::new_v4(),
                name: "默认".to_string(),
                core_type: pp_common::CoreType::SingBox,
                yaml_override: String::new(),
                js_override: r#"function main(c) { c.dns.strategy = "ipv4_only"; return c; }"#
                    .to_string(),
                enabled: true,
            }])
            .unwrap();

        let capture = dir.path().join("core-config-capture.json");
        let core_bin = fake_core_capturing_args(&dir, &capture);
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            format!("http://{addr}"),
            "tok",
            CoreType::SingBox,
            core_bin,
        );
        cfg.mitm_enabled = true;

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        let status = state.status().await;
        let mitm_addr = status.mitm_addr.expect("MITM 应已运行");

        let mut attempts = 0;
        while !capture.exists() && attempts < 100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            attempts += 1;
        }
        assert!(capture.exists(), "假核心应已复制合成配置");
        let core_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&capture).unwrap()).unwrap();

        // 节点经模板分组：n1/n2 保留并被 proxy（select）/ auto（url-test）组引用。
        let outbounds = core_config["outbounds"].as_array().unwrap();
        assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
        assert!(outbounds.iter().any(|o| o["tag"] == "n2"));
        let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
        assert_eq!(proxy["type"], "selector");
        let proxy_out: Vec<&str> = proxy["outbounds"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert!(proxy_out.contains(&"n1") && proxy_out.contains(&"n2"));
        let auto = outbounds.iter().find(|o| o["tag"] == "auto").unwrap();
        assert_eq!(auto["type"], "urltest");

        // 模板替换订阅自带 log/route，JS 复写生效。
        assert_eq!(core_config["log"]["level"], "info");
        assert_eq!(core_config["dns"]["strategy"], "ipv4_only");
        assert_eq!(core_config["route"]["final"], "proxy");

        // compose 注入 inbounds 与 MITM 链。
        let inbounds = core_config["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 2);
        assert_eq!(inbounds[0]["tag"], "main-in");
        assert_eq!(inbounds[1]["tag"], "mitm-return");
        let pp_mitm = outbounds.iter().find(|o| o["tag"] == "pp-mitm").unwrap();
        assert_eq!(pp_mitm["type"], "http");
        assert_eq!(pp_mitm["server_port"], mitm_addr.port());

        state.stop().await;
    }

    /// profiles.json 预置非法 JS 复写 → start 返回 Err；核心未启动、系统代理零调用。
    #[tokio::test]
    async fn start_rolls_back_on_invalid_profile_js_override() {
        let body = r#"{
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } }
            ]
        }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();

        // 预置非法 JS 复写（括号未闭合）的启用模板。
        profile::ProfileStoreV2::new(dir.path().to_path_buf())
            .save(&[profile::Profile {
                id: uuid::Uuid::new_v4(),
                name: "默认".to_string(),
                core_type: pp_common::CoreType::SingBox,
                yaml_override: String::new(),
                js_override: "function main(c) { return c;".to_string(),
                enabled: true,
            }])
            .unwrap();

        let mut cfg = test_config(&dir, format!("http://{addr}"));
        cfg.mitm_enabled = true;
        cfg.system_proxy_enabled = true;

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        assert!(state.start().await.is_err());

        // Profile 构建失败发生在任何组件启动之前：核心未启动、MITM 未启动、
        // 系统代理零调用。
        let status = state.status().await;
        assert!(!status.core_running);
        assert!(status.mitm_addr.is_none());
        assert_eq!(mock.calls(), vec![]);
    }

    /// 注入的自定义 Notifier 到达 ScriptHost：手动运行含 `$notify` 的任务时被记录。
    #[tokio::test]
    async fn injected_notifier_receives_task_notify() {
        let base = spawn_integration_server().await;
        let dir = tempfile::tempdir().unwrap();

        // 预置 remotes.json 并拉取一次写缓存（task.js 含 $notify）。
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
        let notifier = Arc::new(RecordingNotifier::new());
        let mut state = ClientState::with_notifier(cfg, notifier.clone());
        state.start().await.unwrap();

        // 手动运行含 $notify 的任务，验证通知到达注入的 notifier。
        let scheduler = state.scheduler_handle().expect("scheduler should run");
        let out = scheduler.run_now("每日签到").await.unwrap();
        assert_eq!(out.0["code"], 0);
        let calls = notifier.calls();
        assert_eq!(calls.len(), 1, "任务 $notify 应触发一次通知");
        assert_eq!(calls[0].0, "签到成功");

        state.stop().await;
    }

    /// 编译期断言：`ClientState::start` 的 future 为 `Send`（`apply_js_override`
    /// 经 [`ScriptWorker`] 驱动后不再含 rquickjs 非 `Send` 结构，可跨线程 await）。
    #[test]
    fn client_state_start_future_is_send() {
        fn assert_send<T: Send>(_: &T) {}
        let dir = tempfile::tempdir().unwrap();
        let mut state = ClientState::new(test_config(&dir, String::new()));
        let fut = state.start();
        assert_send(&fut);
    }
}
