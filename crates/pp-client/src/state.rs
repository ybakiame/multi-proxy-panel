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
use std::path::Path;
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
use crate::privilege::{TunAuthStatus, tun_auth_status};
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
    /// 当前生效的规则模式（= client.json 持久化值，非法值归一化为 `rule`）。
    pub rule_mode: String,
    /// 本次合成配置的规则条数（sing-box 取 `route.rules`、mihomo 取 `rules`
    /// 数组长度；未运行时为 0）。
    pub rule_count: u64,
    /// Clash 面板 API 地址（核心运行中且 `clash_api_enabled` 时为
    /// `http://127.0.0.1:{clash_api_port}`，否则 `None`）。
    pub clash_api_url: Option<String>,
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
    /// TUN 权限检测函数（默认 [`tun_auth_status`]；测试可注入覆盖以绕过真实权限检查）。
    ///
    /// 仅桌面构建读取（TUN 前置提权检查经 `#[cfg(not(target_os = "android"))]`
    /// 门控，Android 授权走 VpnService 系统弹窗），Android 构建时字段仍保留
    /// （供桌面测试编译与统一构造路径）并允许 dead_code。
    #[cfg_attr(target_os = "android", allow(dead_code))]
    tun_auth_check: Arc<dyn Fn(&Path) -> TunAuthStatus + Send + Sync>,
    /// 本次合成配置的规则条数（start 成功后写入，stop 清零；供 status() 返回）。
    rule_count: u64,
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
            tun_auth_check: Arc::new(tun_auth_status),
            rule_count: 0,
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

    /// 启动：订阅选择（active_subscription_id）→ 覆写解析（订阅关联模板）→ MITM →
    /// 合成配置（MITM 链路）→ 核心 → 系统代理，失败回滚。
    ///
    /// 运行模型（纯关联制）：首页选中的订阅（`active_subscription_id`）唯一生效，
    /// 订阅 `enabled` 仅表示「可被首页选择」；运行时使用的覆写 = 当前生效订阅关联的
    /// 覆写模板（订阅未关联 = 不使用覆写）。订阅选择与模板关联任一异常均返回明确
    /// 错误不启动。未配置选中订阅时回退旧版 Hub 订阅路径（deprecated，无覆写）。
    ///
    /// 启动顺序：先拉取订阅并经 Profile 层生成基础配置（订阅只取节点 → 本地模板 →
    /// 远程 YAML → 本地 YAML → 远程 JS → 本地 JS 叠加复写，远程 URL 拉取失败回退
    /// 缓存、无缓存则跳过并记 warning），MITM 启用时再启动 MITM（拿到监听地址），
    /// 合成配置时注入 [`MitmChain`]（路由规则 + 回流入口），随后启动核心，最后启用
    /// 系统代理（始终指向核心 mixed 主入口，MITM 挂在核心之后由核心路由规则分发）。
    /// Profile 构建失败时尚未启动任何组件，无需回滚。
    pub async fn start(&mut self) -> PanelResult<()> {
        // 总是从磁盘重新加载最新配置：设置页即改即存（8eab048）只写盘
        // client.json，本实例可能持有旧快照；直接用缓存的 config 启动会让
        // 已保存的设置（如 TUN 开关）不生效。data_dir 由应用状态决定，以
        // 实例当前值为准（防 client.json 中的旧路径覆盖）。
        let data_dir = self.config.data_dir.clone();
        let mut loaded = ClientConfig::load(&data_dir)?;
        loaded.data_dir = data_dir;
        self.config = loaded;

        // Android 强制覆盖：libbox 内置核心只支持 sing-box 配置，桌面那套
        // 「选择核心二进制」在 Android 不存在，持久化的 core_type 可能为 mihomo；
        // MITM / 系统代理 / TUN 为桌面专属功能，在 Android 上无对应实现或会
        // 误拦启动（TUN 提权检查、系统代理 stub）。于配置加载后立即归一，避免
        // 按持久化的桌面配置启动失败。
        #[cfg(target_os = "android")]
        apply_android_overrides(&mut self.config);

        // TUN 模式前置提权检查：未授权则拒绝启动并返回明确错误（错误信息以
        // `tun_auth_required` 开头并含二进制路径，前端据此展示授权入口）。
        //
        // Android 授权走 VpnService 系统弹窗（VpnPlugin.prepare），与桌面提权
        // 无关，跳过桌面核心二进制的授权预检（且 Android 无 `core_binary`）。
        #[cfg(not(target_os = "android"))]
        if self.config.tun_enabled {
            match (self.tun_auth_check)(&self.config.core_binary) {
                TunAuthStatus::Authorized => {}
                TunAuthStatus::NeedsAuth => {
                    return Err(PanelError::Client(format!(
                        "tun_auth_required: TUN 模式需要特权，请先授权核心二进制 {}",
                        self.config.core_binary.display()
                    )));
                }
                TunAuthStatus::Unsupported(reason) => {
                    return Err(PanelError::Client(format!(
                        "tun_auth_required: {reason}（核心二进制 {}）",
                        self.config.core_binary.display()
                    )));
                }
            }
        }

        tracing::info!("客户端启动：解析订阅");
        let sub_store = subscription::SubscriptionStore::new(self.config.data_dir.clone());

        // 订阅选择（新模型）：首页选中的订阅（config.active_subscription_id）唯一生效。
        // 选中订阅必须存在且 enabled（可被首页选择）；未配置选中订阅时回退旧版 Hub
        // 订阅路径（hub_url/sub_token 非空，deprecated）。
        //
        // 订阅 → Profile 层：订阅内容只用于提取节点，本地模板生成分组/路由；覆写
        // 模板按纯关联制取当前生效订阅的 `profile_id`（见下方覆写解析）。
        let mut linked_profile_id = None;
        let active_sub = match self.config.active_subscription_id {
            Some(id) => Some(
                sub_store
                    .load()?
                    .into_iter()
                    .find(|s| s.id == id)
                    .ok_or_else(|| {
                        PanelError::Client("所选订阅不存在，请在首页重新选择".to_string())
                    })?,
            ),
            None => None,
        };
        let sub_content = if let Some(sub) = active_sub {
            if !sub.enabled {
                return Err(PanelError::Client(
                    "所选订阅已停用，请在订阅页启用或在首页重新选择".to_string(),
                ));
            }
            linked_profile_id = sub.profile_id;
            let fetch =
                subscription::fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
                    .await?;
            check_subscription_core_compat(fetch.format, self.config.core_type)?;
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
                "未配置选中订阅，回退到旧版 Hub 订阅路径（deprecated）"
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
            return Err(PanelError::Client("请先在首页选择要使用的订阅".to_string()));
        };
        let store = profile::ProfileStoreV2::new(self.config.data_dir.clone());
        // 覆写解析（纯关联制）：运行时使用的覆写 = 当前生效订阅关联的覆写模板；
        // 订阅未关联（或 legacy Hub 回退路径）不使用任何覆写。模板不存在或核心
        // 类型不匹配时返回明确错误。匹配时解析远程复写（拉取/缓存回退/跳过）→
        // 远程为基底、本地叠加的 v2 构建流程。
        let (effective, warnings) = match linked_profile_id {
            Some(pid) => {
                let profiles = store.load()?;
                let linked = profiles.iter().find(|p| p.id == pid).ok_or_else(|| {
                    PanelError::Client("订阅关联的覆写模板不存在，请在订阅页重新关联".to_string())
                })?;
                if linked.core_type != self.config.core_type {
                    return Err(PanelError::Client(format!(
                        "覆写模板「{}」适用于 {}，与当前核心 {} 不匹配，请在首页切换核心或在订阅页调整关联",
                        linked.name,
                        core_type_display_name(linked.core_type),
                        core_type_display_name(self.config.core_type),
                    )));
                }
                profile::resolve_remote_overrides(
                    &self.config.data_dir.join("profile_cache"),
                    linked,
                )
                .await
            }
            None => (profile::EffectiveOverrides::default(), Vec::new()),
        };
        for warning in &warnings {
            tracing::warn!(warning, "profile remote override");
        }
        let profile_cfg =
            profile::build_core_config_v2(self.config.core_type, &sub_content, &effective).await?;

        // MITM 先于核心启动：拿到监听地址才能注入核心路由规则。
        let chain = self.start_mitm_chain().await?;
        // TUN / Clash 面板配置（设置页最高优先级）：在 compose（build_core_config +
        // 复写）之后强制注入，模板/复写中的同名字段以设置为准整体替换。
        //
        // Android 的 TUN 开关与桌面语义解耦：Android 流量由 VpnService（即 TUN）
        // 接管，合成配置必须包含 tun 入站，否则 libbox 不会回调 `openTun()` 建立
        // VPN 接口（核心显示运行中但无接口）。故 Android 恒强制 `tun_enabled=true`；
        // 桌面按用户设置（`apply_android_overrides` 中 `tun_enabled=false` 只作用于
        // 桌面预检与 UI 语义，见该函数注释）。
        let features = core_config::PanelFeatures {
            tun_enabled: panel_features_tun_enabled(
                cfg!(target_os = "android"),
                self.config.tun_enabled,
            ),
            tun_stack: self.config.tun_stack.clone(),
            tun_auto_route: self.config.tun_auto_route,
            clash_api_enabled: self.config.clash_api_enabled,
            clash_api_port: self.config.clash_api_port,
            clash_api_secret: self.config.clash_api_secret.clone(),
            clash_api_ui: self.config.clash_api_ui.clone(),
            rule_mode: self.config.normalized_rule_mode().to_string(),
        };
        let config_json = match self.config.core_type {
            CoreType::SingBox => {
                let mut cfg = match core_config::compose_singbox_config(
                    &profile_cfg,
                    self.config.mixed_port,
                    chain,
                ) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        self.rollback_mitm_started().await;
                        return Err(e);
                    }
                };
                core_config::apply_panel_features(&mut cfg, self.config.core_type, &features);
                cfg
            }
            CoreType::Mihomo => {
                let yaml = serde_yaml::to_string(&profile_cfg)?;
                let mut cfg = match core_config::compose_mihomo_config(
                    &yaml,
                    self.config.mixed_port,
                    chain,
                ) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        self.rollback_mitm_started().await;
                        return Err(e);
                    }
                };
                core_config::apply_panel_features(&mut cfg, self.config.core_type, &features);
                cfg
            }
        };
        self.start_services(&config_json).await?;

        // 运行状态扩展：记录本次合成配置的规则条数（未运行时为 0）。
        self.rule_count = config_json_rule_count(&config_json, self.config.core_type);

        // 核心已启动且 Clash API 开启时，best-effort 推送持久化规则模式：mihomo
        // 启动配置已含顶层 `mode`，此推送冗余但无害；sing-box 无组合层 mode 字段，
        // 完全依赖本次 PATCH 让持久化模式生效。失败仅记 warning，不影响启动。
        let rule_mode = self.config.normalized_rule_mode().to_string();
        if self.config.clash_api_enabled {
            if let Err(e) = core_config::push_clash_mode(
                self.config.clash_api_port,
                &self.config.clash_api_secret,
                &rule_mode,
            )
            .await
            {
                tracing::warn!(
                    error = %e,
                    rule_mode = %rule_mode,
                    "Clash API 推送规则模式失败"
                );
            }
        }
        Ok(())
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
        // 模块 argument 模板替换：用户值（remotes.argument_values）→ 参数默认值
        // （metas.arguments）→ 保留原样；替换结果随 ScriptRule 透传为 $argument。
        let remotes = remote.load().unwrap_or_default();
        let hook_rules =
            crate::remote::apply_argument_templates(merged.scripts, &merged.metas, &remotes);
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
            hook_rules,
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
        // 核心启动日志：Android 核心为内置 libbox（由 Kotlin VpnPlugin 驱动），无独立
        // 二进制；桌面为外部核心二进制，打印其路径。
        #[cfg(target_os = "android")]
        tracing::info!("启动核心（Android 内置 libbox）");
        #[cfg(not(target_os = "android"))]
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
        // 未运行时规则条数清零。
        self.rule_count = 0;
    }

    /// 当前运行状态。
    pub async fn status(&self) -> ClientStatus {
        let core_running = match &self.core {
            Some(c) => c.is_running().await,
            None => false,
        };
        // 核心运行中且 Clash 面板 API 开启时才暴露其地址。
        let clash_api_url = if core_running && self.config.clash_api_enabled {
            Some(format!("http://127.0.0.1:{}", self.config.clash_api_port))
        } else {
            None
        };
        ClientStatus {
            core_running,
            mitm_addr: self.mitm.as_ref().map(|m| m.addr),
            system_proxy: self.sysproxy.is_enabled().await,
            rule_mode: self.config.normalized_rule_mode().to_string(),
            rule_count: self.rule_count,
            clash_api_url,
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

/// Android 启动强制覆盖：libbox 内置核心只支持 sing-box 配置，且桌面专属功能在
/// Android 上无对应实现（或会破坏 VpnService 接管）。
///
/// 各强制项原因：
/// - `core_type = SingBox`：Android 核心为内置 libbox，仅接受 sing-box 配置；
///   持久化的 `core_type` 可能为 mihomo，强制回退避免按 mihomo 合成配置导致启动失败；
/// - `mitm_enabled = false`：MITM 依赖 P3 特性，libbox 不支持，禁用避免链路注入；
/// - `system_proxy_enabled = false`：Android 系统代理为 stub，调用会报错；
/// - `tun_enabled = false`：仅作用于本结构体，驱动桌面 TUN 前置提权检查
///   （[`ClientState::start`] 中 `#[cfg(not(target_os = "android"))]` 门控）与设置页
///   UI 语义；Android 流量由 VpnService（即 TUN）接管，合成配置时必须包含 tun 入站
///   才能触发 libbox 的 `openTun()` 回调建立 VPN 接口，因此 [`ClientState::start`]
///   构造 [`core_config::PanelFeatures`] 时在 Android 上经
///   [`panel_features_tun_enabled`] 强制 `tun_enabled=true`（本处的 false 不参与
///   合成配置）。
///
/// 仅在 Android 构建时由 [`ClientState::start`] 调用；桌面构建编译该函数仅供
/// 单元测试验证语义。
#[cfg(any(test, target_os = "android"))]
fn apply_android_overrides(config: &mut ClientConfig) {
    config.core_type = CoreType::SingBox;
    config.mitm_enabled = false;
    config.system_proxy_enabled = false;
    config.tun_enabled = false;
    tracing::info!(
        "Android 强制覆盖：核心类型 = sing-box（libbox），禁用 MITM / 系统代理 / TUN（仅桌面语义，合成配置仍强制 tun 入站）"
    );
}

/// PanelFeatures 的 TUN 开关：Android 由 VpnService（libbox）接管流量，合成配置
/// 必须恒含 tun 入站才能建立 VPN 接口；桌面按用户设置原样透传。
fn panel_features_tun_enabled(is_android: bool, tun_enabled: bool) -> bool {
    if is_android { true } else { tun_enabled }
}

/// 订阅格式 ↔ 核心类型绑定校验。
///
/// - [`SubFormat::SingBoxJson`] 仅支持 sing-box 核心（其节点为 sing-box JSON，跨格式
///   转 mihomo 有信息丢失，且此路径在「订阅绑定格式」决策后已废弃）；
/// - [`SubFormat::ClashYaml`] 仅支持 mihomo 核心（历史 bug：clash 订阅节点转 sing-box
///   outbound 时丢失 TLS 块导致 sing-box `initialize outbound: TLS required` FATAL）；
/// - [`SubFormat::ShareLinks`] 双核心皆可。
///
/// 不匹配时返回明确错误（含检测到的订阅格式与当前核心类型），核心不启动。
fn check_subscription_core_compat(
    format: subscription::SubFormat,
    core_type: CoreType,
) -> PanelResult<()> {
    let compatible = match format {
        subscription::SubFormat::ShareLinks => true,
        subscription::SubFormat::SingBoxJson => core_type == CoreType::SingBox,
        subscription::SubFormat::ClashYaml => core_type == CoreType::Mihomo,
    };
    if compatible {
        return Ok(());
    }
    let (format_name, supported_core) = match format {
        subscription::SubFormat::ClashYaml => ("clash", "mihomo"),
        subscription::SubFormat::SingBoxJson => ("sing-box", "sing-box"),
        subscription::SubFormat::ShareLinks => {
            unreachable!("ShareLinks 双核心皆可，不会走到不匹配分支")
        }
    };
    Err(PanelError::Client(format!(
        "订阅格式为 {format_name}，仅支持 {supported_core} 核心，当前核心类型为 {core_type}，请在设置中切换核心类型"
    )))
}

/// 核心类型的用户可见展示名（`sing-box` / `mihomo`），用于覆写匹配错误信息。
pub fn core_type_display_name(core_type: CoreType) -> &'static str {
    match core_type {
        CoreType::SingBox => "sing-box",
        CoreType::Mihomo => "mihomo",
    }
}

/// 计算合成配置的规则条数：sing-box 取 `route.rules` 数组长度，mihomo 取顶层
/// `rules` 数组长度（数组缺失按 0）。
fn config_json_rule_count(config_json: &serde_json::Value, core_type: CoreType) -> u64 {
    match core_type {
        CoreType::SingBox => config_json
            .get("route")
            .and_then(|r| r.get("rules"))
            .and_then(|rules| rules.as_array())
            .map_or(0, |rules| rules.len() as u64),
        CoreType::Mihomo => config_json
            .get("rules")
            .and_then(|rules| rules.as_array())
            .map_or(0, |rules| rules.len() as u64),
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
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            hub_url,
            "tok",
            CoreType::SingBox,
            fake_core_script(dir),
        );
        // start 现在总是从磁盘 reload 配置：测试配置必须先落盘 client.json。
        cfg.save().unwrap();
        cfg
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
        cfg.save().unwrap();

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
        cfg.save().unwrap();
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
        cfg.save().unwrap();
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
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        let status = state.status().await;
        assert!(status.core_running, "mihomo 核心应启动");
        assert_eq!(status.rule_mode, "rule", "默认规则模式为 rule");
        // yaml 含 1 条 `MATCH,DIRECT` 规则；clash_api 默认关闭 → 无 API 地址。
        assert_eq!(status.rule_count, 1);
        assert_eq!(status.clash_api_url, None);

        state.stop().await;
        let status = state.status().await;
        assert!(!status.core_running);
    }

    /// 规则模式 + Clash API 集成：启动成功且 clash_api_enabled 时，best-effort 经
    /// `PATCH /configs` 推送持久化模式（对 sing-box 而言是唯一生效通道）；status()
    /// 返回 rule_mode / rule_count / clash_api_url 新字段。
    #[tokio::test]
    async fn start_pushes_rule_mode_via_clash_api_when_enabled() {
        // 假 Clash API server：接收 PATCH /configs 并记录请求体。
        let captured = Arc::new(std::sync::Mutex::new(None));
        let captured_ref = Arc::clone(&captured);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let clash_addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route(
            "/configs",
            axum::routing::patch(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    let bytes = axum::body::to_bytes(req.into_body(), 1024).await.unwrap();
                    *captured_ref.lock().unwrap() = Some(bytes.to_vec());
                    axum::http::StatusCode::NO_CONTENT
                },
            ),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // 订阅含 1 条 route 规则（供 rule_count 断言）。
        let sub_body = r#"{
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": { "final": "direct", "rules": [{"action": "sniff"}] }
        }"#;
        let addr = spawn_server(StatusCode::OK, sub_body).await;
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = test_config(&dir, format!("http://{addr}"));
        cfg.clash_api_enabled = true;
        cfg.clash_api_port = clash_addr.port();
        cfg.clash_api_secret = "sekret".to_string();
        cfg.rule_mode = "global".to_string();
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        state.start().await.unwrap();

        // 启动时经 Clash API 推送持久化模式（rule_mode=global）。
        let body = captured.lock().unwrap().clone().expect("应收到 PATCH 请求");
        assert_eq!(body, br#"{"mode":"global"}"#);

        // 状态扩展字段。
        let status = state.status().await;
        assert_eq!(status.rule_mode, "global");
        assert_eq!(status.rule_count, 1);
        assert_eq!(
            status.clash_api_url,
            Some(format!("http://127.0.0.1:{}", clash_addr.port()))
        );

        // 停止后规则条数清零、API 地址消失。
        state.stop().await;
        let status = state.status().await;
        assert_eq!(status.rule_count, 0);
        assert_eq!(status.clash_api_url, None);
    }

    /// 项 1：clash 格式订阅 + sing-box 核心 → start 返回明确的格式/核心不匹配错误，
    /// 核心不启动、系统代理零调用。
    #[tokio::test]
    async fn start_rejects_clash_format_with_singbox_core() {
        const YAML: &str = "port: 7890\nproxies:\n  - name: n1\n    type: ss\n    server: example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: pw\nrules:\n  - MATCH,DIRECT\n";
        let addr = spawn_server(StatusCode::OK, YAML).await;
        let dir = tempfile::tempdir().unwrap();
        // 通用订阅路径：subscriptions.json 指向本地 server（clash 格式），
        // client.json 选中该订阅（新模型：选中订阅唯一生效）。
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("clash-sub", &format!("http://{addr}/sub"), true, None)
            .unwrap();

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.mitm_enabled = false;
        cfg.system_proxy_enabled = true;
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        let err = state.start().await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("clash"), "应包含检测到的格式 clash: {msg}");
        assert!(msg.contains("mihomo"), "应包含受支持的核心 mihomo: {msg}");
        assert!(msg.contains("切换核心类型"), "应提示切换核心类型: {msg}");

        // 核心未启动、系统代理零调用。
        let status = state.status().await;
        assert!(!status.core_running);
        assert_eq!(mock.calls(), vec![]);
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
        // subscriptions.json 指向本地 server，client.json 选中该订阅。
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("local", &format!("{base}/sub"), true, None)
            .unwrap();

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.mitm_enabled = false;
        cfg.save().unwrap();

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
        cfg.save().unwrap();

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

        // 运行状态扩展：本次合成配置含 1 条 MITM 白名单路由规则。
        let status = state.status().await;
        assert_eq!(status.rule_count, 1);

        state.stop().await;
        let status = state.status().await;
        assert!(status.mitm_addr.is_none());
        assert!(!status.core_running);
    }

    /// Profile 层集成：订阅（2 节点 + selector/direct 组）经模板分组、订阅关联的
    /// 覆写模板 JS 复写生效，compose 正常注入 inbounds 与 MITM 链。
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

        // 预置 profiles.json：一个 SingBox 模板，其 js_override 修改 dns.strategy。
        let profile_id = uuid::Uuid::new_v4();
        profile::ProfileStoreV2::new(dir.path().to_path_buf())
            .save(&[profile::Profile {
                id: profile_id,
                name: "默认".to_string(),
                core_type: pp_common::CoreType::SingBox,
                yaml_override: String::new(),
                js_override: r#"function main(c) { c.dns.strategy = "ipv4_only"; return c; }"#
                    .to_string(),
                yaml_url: None,
                js_url: None,
            }])
            .unwrap();

        // 订阅关联该模板（纯关联制）：subscriptions.json 指向本地 server 并关联
        // profile_id，client.json 选中该订阅。
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("local", &format!("http://{addr}/sub"), true, None)
            .unwrap();
        store.set_profile_id(sub.id, Some(profile_id)).unwrap();

        let capture = dir.path().join("core-config-capture.json");
        let core_bin = fake_core_capturing_args(&dir, &capture);
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            core_bin,
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.mitm_enabled = true;
        cfg.save().unwrap();

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

    /// 订阅关联的覆写模板预置非法 JS 复写 → start 返回 Err；核心未启动、系统代理零调用。
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

        // 预置非法 JS 复写（括号未闭合）的模板，并关联到选中订阅（纯关联制）。
        let profile_id = uuid::Uuid::new_v4();
        profile::ProfileStoreV2::new(dir.path().to_path_buf())
            .save(&[profile::Profile {
                id: profile_id,
                name: "默认".to_string(),
                core_type: pp_common::CoreType::SingBox,
                yaml_override: String::new(),
                js_override: "function main(c) { return c;".to_string(),
                yaml_url: None,
                js_url: None,
            }])
            .unwrap();
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("local", &format!("http://{addr}/sub"), true, None)
            .unwrap();
        store.set_profile_id(sub.id, Some(profile_id)).unwrap();

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.mitm_enabled = true;
        cfg.system_proxy_enabled = true;
        cfg.save().unwrap();

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

    /// 新模型：未选中订阅且无 legacy Hub 配置 → start 返回「请先在首页选择要使用的订阅」。
    #[tokio::test]
    async fn start_requires_active_subscription_selection() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.mitm_enabled = false;
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        let err = state.start().await.unwrap_err();
        assert!(err.to_string().contains("请先在首页选择"), "{err}");
        assert_eq!(mock.calls(), vec![]);
    }

    /// 新模型：active_subscription_id 指向已停用订阅 → 明确错误，核心不启动。
    #[tokio::test]
    async fn start_rejects_disabled_selected_subscription() {
        let addr = spawn_server(StatusCode::OK, "{}").await;
        let dir = tempfile::tempdir().unwrap();
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("off", &format!("http://{addr}/sub"), false, None)
            .unwrap();

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.mitm_enabled = false;
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        let err = state.start().await.unwrap_err();
        assert!(err.to_string().contains("已停用"), "{err}");
        assert_eq!(mock.calls(), vec![]);
    }

    /// 新模型：订阅关联的覆写模板核心类型与当前核心不匹配 → 明确错误（含 sing-box /
    /// mihomo 展示名），核心不启动。
    #[tokio::test]
    async fn start_rejects_profile_core_type_mismatch() {
        let body = r#"{ "outbounds": [] }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();

        let profile_id = uuid::Uuid::new_v4();
        profile::ProfileStoreV2::new(dir.path().to_path_buf())
            .save(&[profile::Profile {
                id: profile_id,
                name: "mihomo 模板".to_string(),
                core_type: pp_common::CoreType::Mihomo,
                yaml_override: String::new(),
                js_override: String::new(),
                yaml_url: None,
                js_url: None,
            }])
            .unwrap();
        let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("local", &format!("http://{addr}/sub"), true, None)
            .unwrap();
        store.set_profile_id(sub.id, Some(profile_id)).unwrap();

        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            fake_core_script(&dir),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.mitm_enabled = false;
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        let err = state.start().await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("不匹配"), "{msg}");
        assert!(msg.contains("sing-box") && msg.contains("mihomo"), "{msg}");
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
        cfg.save().unwrap();
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

    /// 项 1 回归：start 总是从磁盘重新加载配置。
    ///
    /// 磁盘 client.json `tun_enabled=false`，但实例缓存旧快照 `tun_enabled=true`
    /// （模拟 UI 即改即存前已创建的 `ClientState`）；start 后核心收到的合成配置
    /// 必须不含 tun 入站（用户实测「设置中关闭 TUN，启动仍注入 tun-in」）。
    #[tokio::test]
    async fn start_reloads_disk_config_so_tun_toggle_takes_effect() {
        let body = r#"{
            "log": {"level": "info"},
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } }
            ],
            "route": { "final": "direct" }
        }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();

        // 磁盘配置：TUN 已关闭（用户实测场景）。
        let capture = dir.path().join("core-config-capture.json");
        let core_bin = fake_core_capturing_args(&dir, &capture);
        let mut disk = ClientConfig::new(
            dir.path().to_path_buf(),
            format!("http://{addr}"),
            "tok",
            CoreType::SingBox,
            core_bin,
        );
        disk.tun_enabled = false;
        disk.save().unwrap();

        // 缓存的旧快照：TUN 仍开启（模拟 ClientState 在用户关闭 TUN 前已创建）。
        let mut stale = disk.clone();
        stale.tun_enabled = true;

        let mock = Arc::new(MockSystemProxy::new());
        // 磁盘 tun_enabled=false：reload 生效后不触发 TUN 前置提权检查，
        // 用默认权限检测即可（若 reload 失效，start 会因 NeedsAuth 报错失败）。
        let mut state = ClientState::with_system_proxy(stale, mock.clone());
        state.start().await.unwrap();

        let mut attempts = 0;
        while !capture.exists() && attempts < 100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            attempts += 1;
        }
        assert!(capture.exists(), "假核心应已复制合成配置");
        let core_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&capture).unwrap()).unwrap();
        let inbounds = core_config["inbounds"].as_array().unwrap();
        assert!(
            !inbounds.iter().any(|i| i["type"] == "tun"),
            "磁盘 tun_enabled=false 时不应注入 tun 入站: {core_config}"
        );

        state.stop().await;
    }

    /// 项 2：`tun_enabled=true` 但核心未授权 → start 返回 `tun_auth_required` 错误，
    /// 核心不启动、系统代理零调用（前端据此展示授权入口）。
    #[tokio::test]
    async fn start_requires_tun_authorization_when_tun_enabled() {
        let body = r#"{
            "log": {"level": "info"},
            "outbounds": [{"type": "direct"}]
        }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = test_config(&dir, format!("http://{addr}"));
        cfg.tun_enabled = true;
        cfg.save().unwrap();
        let binary = cfg.core_binary.clone();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState::with_system_proxy(cfg, mock.clone());
        let err = state.start().await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("tun_auth_required"),
            "应含 tun_auth_required 标记: {msg}"
        );
        assert!(
            msg.contains(binary.to_string_lossy().as_ref()),
            "应含核心二进制路径: {msg}"
        );

        // 核心未启动、系统代理零调用。
        let status = state.status().await;
        assert!(!status.core_running);
        assert_eq!(mock.calls(), vec![]);
    }

    /// Android 语义：`apply_android_overrides` 强制 sing-box 核心并禁用桌面专属
    /// 功能。桌面构建无法执行 Android 路径，故抽取纯函数后在桌面测试其语义。
    ///
    /// 注意 `tun_enabled = false` 只作用于桌面预检与 UI 语义：Android 合成配置经
    /// `panel_features_tun_enabled` 恒强制 `tun_enabled=true`（见下测试），二者不冲突。
    #[test]
    fn apply_android_overrides_forces_singbox_and_disables_desktop_features() {
        let mut cfg = ClientConfig {
            core_type: CoreType::Mihomo,
            mitm_enabled: true,
            system_proxy_enabled: true,
            tun_enabled: true,
            ..ClientConfig::default()
        };
        apply_android_overrides(&mut cfg);
        assert_eq!(
            cfg.core_type,
            CoreType::SingBox,
            "Android 强制 sing-box 核心"
        );
        assert!(!cfg.mitm_enabled, "Android 禁用 MITM");
        assert!(!cfg.system_proxy_enabled, "Android 禁用系统代理");
        assert!(!cfg.tun_enabled, "Android 禁用 TUN（桌面语义）");
    }

    /// Android 合成配置的 TUN 开关恒为 true（libbox 需要 tun 入站才回调 openTun
    /// 建立 VPN 接口）；桌面按用户设置原样透传。
    #[test]
    fn panel_features_tun_enabled_forces_true_on_android_only() {
        assert!(
            panel_features_tun_enabled(true, false),
            "Android 恒开启 TUN"
        );
        assert!(panel_features_tun_enabled(true, true), "Android 恒开启 TUN");
        assert!(
            !panel_features_tun_enabled(false, false),
            "桌面关闭 TUN 时保持关闭"
        );
        assert!(
            panel_features_tun_enabled(false, true),
            "桌面开启 TUN 时保持开启"
        );
    }

    /// 项 2：`tun_enabled=true` 且已授权（注入 Authorized 检测）→ 注入 tun 入站。
    #[tokio::test]
    async fn start_injects_tun_inbound_when_authorized() {
        let body = r#"{
            "log": {"level": "info"},
            "outbounds": [{"type": "direct"}]
        }"#;
        let addr = spawn_server(StatusCode::OK, body).await;
        let dir = tempfile::tempdir().unwrap();

        let capture = dir.path().join("core-config-capture.json");
        let core_bin = fake_core_capturing_args(&dir, &capture);
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            format!("http://{addr}"),
            "tok",
            CoreType::SingBox,
            core_bin,
        );
        cfg.tun_enabled = true;
        cfg.save().unwrap();

        let mock = Arc::new(MockSystemProxy::new());
        let mut state = ClientState {
            config: cfg,
            core: None,
            mitm: None,
            sysproxy: mock,
            notifier: Arc::new(TracingNotifier::new()),
            recorder: Arc::new(MemoryRecorder::new(2048)),
            scheduler: None,
            tun_auth_check: Arc::new(|_| TunAuthStatus::Authorized),
            rule_count: 0,
        };
        state.start().await.unwrap();

        let mut attempts = 0;
        while !capture.exists() && attempts < 100 {
            tokio::time::sleep(Duration::from_millis(50)).await;
            attempts += 1;
        }
        assert!(capture.exists(), "假核心应已复制合成配置");
        let core_config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&capture).unwrap()).unwrap();
        let inbounds = core_config["inbounds"].as_array().unwrap();
        assert!(
            inbounds.iter().any(|i| i["type"] == "tun"),
            "tun_enabled=true 且已授权时应注入 tun 入站: {core_config}"
        );

        state.stop().await;
    }
}
