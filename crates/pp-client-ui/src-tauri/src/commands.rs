//! Tauri 命令层：把 `pp-client` 内部类型包装为 serde 视图结构，供前端调用。
//!
//! 视图类型（`*View`）是独立的 serde 简单结构，避免把内部类型直接暴露给
//! 前端；内部类型与视图的转换通过字段映射与 `serde_json` 往返完成。

use std::path::PathBuf;
use std::sync::Arc;

use pp_client::{
    apply_panel_features, build_core_config_v2, compose_mihomo_config, compose_singbox_config,
    detect_resource_from_url, fetch_subscription_with_ua, parse_config_meta,
    resolve_remote_overrides, ClientConfig, ClientState, ConfigMeta, EffectiveOverrides,
    PanelFeatures, Profile, ProfileStoreV2, RemoteKind, RemoteManager, RemoteResource, SubContent,
    SubFormat, Subscription, SubscriptionFetcher, SubscriptionStore,
};
use pp_common::CoreType;
use pp_mitm::{CaStore, TrafficRecorder};
use pp_script::{ScriptDialect, TaskScriptView};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::state::AppState;

/// 通过 tauri-plugin-notification 发送 OS 桌面通知的 [`pp_script::Notifier`]。
///
/// 通知发送失败时回退为 `tracing::warn` 日志，不阻断脚本执行。
pub struct TauriNotifier {
    app: tauri::AppHandle,
}

impl TauriNotifier {
    /// 使用应用句柄创建通知器。
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl pp_script::Notifier for TauriNotifier {
    fn notify(&self, title: &str, subtitle: &str, body: &str, _options: Option<serde_json::Value>) {
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = self
            .app
            .notification()
            .builder()
            .title(title)
            .body(format!("{subtitle}\n{body}"))
            .show()
        {
            tracing::warn!(error = %e, "发送桌面通知失败");
        }
    }
}

/// 客户端配置的对外视图（serde 简单结构，避免直接暴露内部类型）。
///
/// 注意：所有 `*View` 结构体按字段名原样（snake_case）序列化，与前端
/// `src/api.ts` 的 TS 类型逐字段对齐；曾一度使用 `rename_all = "camelCase"`
/// 导致前端读取 `mitm_hostnames` 等字段全部为 `undefined` 而崩溃。
///
/// `#[serde(default)]`：前端回传 payload 缺失任一字段（如旧版前端未发送开关
/// 布尔值）时按默认值补齐，避免整次保存因反序列化失败而中断。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ClientConfigView {
    /// 数据目录（展示用；持久化路径由应用状态决定）。
    pub data_dir: String,
    pub hub_url: String,
    pub sub_token: String,
    /// 首页选中的生效订阅（`data_dir/subscriptions.json` 中的订阅 id；`null` = 未选中）。
    pub active_subscription_id: Option<String>,
    /// 核心类型：`singbox` / `mihomo`（与 `pp_common::CoreType` 的 serde 表示一致）。
    pub core_type: String,
    pub core_binary: String,
    pub mixed_port: u16,
    pub mitm_enabled: bool,
    pub mitm_hostnames: Vec<String>,
    /// MITM 脚本方言：`Surge` / `Loon`。
    pub mitm_script_dialect: String,
    pub system_proxy_enabled: bool,
    /// 是否启用 TUN 虚拟网卡（需 root/管理员权限）。
    pub tun_enabled: bool,
    /// TUN 协议栈：`gvisor` / `system` / `mixed`。
    pub tun_stack: String,
    /// TUN 自动路由。
    pub tun_auto_route: bool,
    /// 是否启用 Clash 面板 API。
    pub clash_api_enabled: bool,
    /// Clash 面板 API 监听端口。
    pub clash_api_port: u16,
    /// Clash 面板 API 密钥（空串 = 不鉴权）。
    pub clash_api_secret: String,
    /// Clash 面板 UI 选择：`yacd` / `zashboard` / `metacubexd`（默认 `zashboard`）。
    pub clash_api_ui: String,
    /// GitHub 代理前缀（如 `https://gh-proxy.com`；空串 = 直连 GitHub）。
    pub github_proxy_prefix: String,
    /// 远程资源拉取是否经本地核心 mixed 端口代理。
    pub fetch_via_local_proxy: bool,
    /// 规则模式：`rule` / `global` / `direct`（默认 `rule`）。
    ///
    /// 非法值在 `into_config` 时原样落盘，由 `pp_client::ClientConfig` 的
    /// `normalized_rule_mode()` 读取侧归一化兜底（合成与热切换均回退 `rule`）。
    pub rule_mode: String,
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
        }
    }
}

impl ClientConfigView {
    /// 从内部配置构造视图。
    fn from_config(cfg: &ClientConfig) -> Self {
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
        }
    }

    /// 转为内部配置；`data_dir` 由应用状态决定（视图中的 `data_dir` 仅作展示）。
    fn into_config(self, data_dir: &std::path::Path) -> Result<ClientConfig, String> {
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
        });
        serde_json::from_value::<ClientConfig>(value).map_err(|e| e.to_string())
    }
}

/// 客户端运行状态的对外视图。
#[derive(Debug, Clone, Serialize, Default)]
pub struct ClientStatusView {
    pub core_running: bool,
    pub mitm_addr: Option<String>,
    pub system_proxy: bool,
    /// 当前生效的规则模式（`rule` / `global` / `direct`）。
    pub rule_mode: String,
    /// 本次合成配置的规则条数（未运行时为 0）。
    pub rule_count: u64,
    /// Clash 面板 API 地址（核心运行中且 clash_api_enabled 时，否则 `None`）。
    pub clash_api_url: Option<String>,
}

impl ClientStatusView {
    fn from_status(status: &pp_client::ClientStatus) -> Self {
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

/// 一条流量记录的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct TrafficRecordView {
    pub id: String,
    pub method: String,
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Option<String>,
    pub timestamp: String,
    pub duration_ms: u64,
}

impl TrafficRecordView {
    /// 从内部流量记录构造视图。
    fn from_record(rec: &pp_mitm::TrafficRecord) -> Self {
        Self {
            id: rec.id.to_string(),
            method: rec.method.clone(),
            url: rec.url.clone(),
            request_headers: rec.request_headers.clone(),
            request_body: rec.request_body.clone(),
            response_status: rec.response_status,
            response_headers: rec.response_headers.clone(),
            response_body: rec.response_body.clone(),
            timestamp: rec.timestamp.to_rfc3339(),
            duration_ms: rec.duration_ms,
        }
    }
}

/// 读取当前配置（`data_dir/client.json` 不存在时返回默认值）。
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

/// 保存配置的返回视图（携带非阻塞提示）。
#[derive(Debug, Clone, Serialize, Default)]
pub struct SaveConfigView {
    /// 非阻塞提示（core_type 联动后缺少本地核心等）。
    pub warning: Option<String>,
}

/// 保存配置实现（命令层可测试的纯逻辑）。
///
/// - **基本设置可随时保存**：订阅管理已独立（`hub_url` / `sub_token` 字段仅保留
///   兼容，不再校验空值，也不产生缺失提示）；
/// - **core_type 联动本地核心**：`core_type` 变更且当前 `core_binary` 不属于该
///   类型时自动填入该类型首选本地核心（版本最高已下载 → 系统探测）；
///   找不到时保留原路径并返回 `warning` 提示去核心管理下载。仅桌面端生效，
///   Android 核心为内置 panelcore 合并绑定，跳过该联动（无本地二进制管理）。
fn save_config_impl(
    data_dir: &std::path::Path,
    cfg: ClientConfigView,
) -> Result<SaveConfigView, String> {
    // Android 下联动块整体被编译排除，`config` / `warnings` 不再被就地修改。
    #[cfg_attr(target_os = "android", allow(unused_mut))]
    let mut config = cfg.into_config(data_dir)?;
    #[cfg_attr(target_os = "android", allow(unused_mut))]
    let mut warnings: Vec<String> = Vec::new();

    // core_type 联动本地核心二进制（仅桌面/非 Android：Android 核心为内置
    // panelcore 合并绑定，无本地二进制下载管理语义，跳过以避免误导性
    // “未找到本地核心”警告）。
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

/// 保存配置（`hub_url` / `sub_token` 已退役，不再校验也不产生提示；`core_type`
/// 变更时联动本地核心二进制）。
#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    cfg: ClientConfigView,
) -> Result<SaveConfigView, String> {
    save_config_impl(&state.data_dir, cfg)
}

/// 启动代理（无运行状态时先基于已保存配置新建）。
///
/// `ClientState` 注入 [`TauriNotifier`]：脚本 `$notify` / `$notification` 通过
/// tauri-plugin-notification 发送 OS 桌面通知。
///
/// `ClientState::start`（内部 `build_core_config` 的 JS 复写经 pp-script
/// `ScriptWorker` 驱动）返回 `Send` future，可直接在 Tauri 命令（要求 `Send`
/// future）中 `await`。
#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    if lock.is_none() {
        let cfg = ClientConfig::load(&state.data_dir)
            .map_err(|e| format!("未找到已保存的配置（{e}），请先保存配置"))?;
        *lock = Some(ClientState::with_notifier(
            cfg,
            Arc::new(TauriNotifier::new(app)),
        ));
    }
    let client = lock
        .as_mut()
        .ok_or_else(|| "客户端状态初始化失败".to_string())?;
    client.start().await.map_err(|e| format!("启动失败: {e}"))?;
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// 停止代理。
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

/// TUN 提权状态（基于当前配置的 `core_binary`）。
///
/// 返回前端字符串：`authorized` / `needs_auth` / `unsupported:<reason>`。
/// 启动流程在 `tun_enabled` 时前置检查，未授权返回 `tun_auth_required` 错误，
/// 前端据此展示授权按钮。
#[tauri::command]
pub async fn tun_auth_status(state: State<'_, AppState>) -> Result<String, String> {
    let cfg = ClientConfig::load(&state.data_dir).map_err(|e| format!("读取配置失败: {e}"))?;
    Ok(pp_client::tun_auth_status(&cfg.core_binary).as_frontend_str())
}

/// 执行 TUN 提权（Linux `pkexec setcap` / macOS `osascript` setuid / Windows
/// 提示以管理员身份重启），返回授权后的最新状态。
#[tauri::command]
pub async fn authorize_tun(state: State<'_, AppState>) -> Result<String, String> {
    let cfg = ClientConfig::load(&state.data_dir).map_err(|e| format!("读取配置失败: {e}"))?;
    pp_client::authorize_tun(&cfg.core_binary).map_err(|e| e.to_string())?;
    Ok(pp_client::tun_auth_status(&cfg.core_binary).as_frontend_str())
}

/// 运行平台信息的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct PlatformInfoView {
    /// 运行平台：`android` / `linux` / `windows` / `macos`。
    pub os: String,
}

/// 查询运行平台（前端据此隐藏桌面专属开关：Android 由 VpnService 接管，
/// 系统代理 / MITM / TUN 桌面开关无效）。
#[tauri::command]
pub fn platform_info() -> PlatformInfoView {
    PlatformInfoView {
        os: std::env::consts::OS.to_string(),
    }
}

/// 请求系统 VPN 授权（仅 Android）：经 `vpn` 插件调用 Kotlin
/// `VpnPlugin.prepare`（`VpnService.prepare` 的 Activity 跳转），授权结果以
/// Promise resolve/reject 返回。桌面端无此命令（`generate_handler` 中
/// `#[cfg(target_os = "android")]` 排除）。
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn request_vpn_permission() -> Result<(), String> {
    let handle = crate::core_bridge::vpn_plugin_handle()
        .ok_or_else(|| "VPN 插件未初始化，请重启应用后重试".to_string())?;
    handle
        .run_mobile_plugin_async::<serde_json::Value>("prepare", ())
        .await
        .map(|_| ())
        .map_err(|e| format!("VPN 授权失败: {e}"))
}

/// 读取最近一次 VPN 启动失败原因（仅 Android）：经 `vpn` 插件 `isRunning` 命令
/// 读取 Kotlin `ProxyVpnService.lastError`（服务启动失败时记录，成功启动后清空）。
///
/// 插件未初始化 / 命令失败 / 无失败记录时返回 `None`（前端轮询展示「启动失败」
/// Alert 只在有值时出现）。桌面端无此命令（`generate_handler` 中
/// `#[cfg(target_os = "android")]` 排除）。
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn vpn_last_error() -> Option<String> {
    let handle = crate::core_bridge::vpn_plugin_handle()?;
    let resp = handle
        .run_mobile_plugin_async::<crate::core_bridge::IsRunningResponse>("isRunning", ())
        .await
        .ok()?;
    resp.last_error
}

/// 查询代理运行状态。
#[tauri::command]
pub async fn proxy_status(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(ClientStatusView::default());
    };
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// 无运行客户端实例时的状态视图（`rule_mode` 取已落盘的归一化值）。
fn idle_status_view(data_dir: &std::path::Path) -> ClientStatusView {
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

/// `set_rule_mode` 的持久化部分（命令层可测试的纯逻辑）：校验模式合法并写入
/// `client.json`。
fn set_rule_mode_persist(data_dir: &std::path::Path, mode: &str) -> Result<(), String> {
    match mode {
        "rule" | "global" | "direct" => {}
        _ => return Err("无效的规则模式".to_string()),
    }
    let mut config = ClientConfig::load(data_dir).map_err(|e| format!("读取配置失败: {e}"))?;
    config.rule_mode = mode.to_string();
    config.save().map_err(|e| format!("保存配置失败: {e}"))
}

/// 设置规则模式（`rule` / `global` / `direct`）：写入 `client.json` 持久化。
///
/// 客户端已创建且核心运行中、Clash API 开启时，best-effort 经 `PATCH /configs`
/// 热切换（失败不向用户报错，仅记 warning，当前模式以返回的 status 为准）。
/// 返回值携带最新 `ClientStatusView`。
#[tauri::command]
pub async fn set_rule_mode(
    state: State<'_, AppState>,
    mode: String,
) -> Result<ClientStatusView, String> {
    set_rule_mode_persist(&state.data_dir, &mode)?;
    let mut lock = state.client.lock().await;
    let Some(client) = lock.as_mut() else {
        return Ok(idle_status_view(&state.data_dir));
    };
    // 同步实例内存配置：status() 读取它返回最新模式（下次 start 亦从磁盘 reload）。
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
            tracing::warn!(error = %e, mode = %mode, "Clash API 热切换规则模式失败");
        }
    }
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// 查询 MITM 流量记录。
///
/// 有运行中的 `ClientState` 时通过 `ClientState::recorder()` 取回
/// `MemoryRecorder` 并按时间序映射为视图；未启动时返回空列表。
#[tauri::command]
pub async fn list_traffic(state: State<'_, AppState>) -> Result<Vec<TrafficRecordView>, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(Vec::new());
    };
    let records = client.recorder().list();
    Ok(records.iter().map(TrafficRecordView::from_record).collect())
}

/// MITM CA 证书的对外视图（供客户端信任指引展示）。
#[derive(Debug, Clone, Serialize)]
pub struct MitmCaView {
    /// `ca.crt` 的绝对路径（供用户导入系统/浏览器信任库）。
    pub path: String,
    /// PEM 格式的根证书内容。
    pub pem: String,
}

/// 获取 MITM CA 证书（`data_dir/certs/ca.{crt,key}`；不存在时由
/// [`pp_mitm::FileCaStore`] 生成并落盘，已存在时不重复生成）。
#[tauri::command]
pub fn get_mitm_ca(state: State<'_, AppState>) -> Result<MitmCaView, String> {
    get_mitm_ca_impl(&state.data_dir)
}

/// `get_mitm_ca` 的具体实现（命令层可测试的纯逻辑）。
fn get_mitm_ca_impl(data_dir: &std::path::Path) -> Result<MitmCaView, String> {
    let store = pp_mitm::FileCaStore::new(data_dir.join("certs"));
    let material = store
        .load_or_generate()
        .map_err(|e| format!("读取 MITM CA 失败: {e}"))?;
    Ok(MitmCaView {
        path: data_dir
            .join("certs")
            .join("ca.crt")
            .to_string_lossy()
            .into_owned(),
        pem: material.cert_pem,
    })
}

/// 一条远程订阅资源的对外视图。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteResourceView {
    pub name: String,
    pub url: String,
    /// 资源类型：`Script` / `Snippet`。
    pub kind: String,
    /// 脚本方言：`Surge` / `Loon`。
    pub dialect: String,
    /// 资源描述（可选；`None` = 未配置）。
    pub description: Option<String>,
    pub update_interval_secs: u64,
    pub enabled: bool,
    /// 用户为模块参数配置的值 `(key, value)`（对应 `#!arguments=` 声明的键）。
    pub argument_values: Vec<(String, String)>,
    /// 资源图标 URL（可选；嗅探结果预填）。
    pub icon: Option<String>,
    /// 模块参数声明（`#!arguments=` / Loon `[Argument]` 段；旧前端回传缺省为空）。
    #[serde(default)]
    pub arguments: Vec<ArgSpecView>,
}

impl RemoteResourceView {
    fn from_remote(remote: &RemoteResource) -> Self {
        Self {
            name: remote.name.clone(),
            url: remote.url.clone(),
            kind: serde_json::to_value(remote.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            dialect: serde_json::to_value(remote.dialect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            description: remote.description.clone(),
            update_interval_secs: remote.update_interval_secs,
            enabled: remote.enabled,
            argument_values: remote.argument_values.clone(),
            icon: remote.icon.clone(),
            arguments: remote.arguments.iter().map(ArgSpecView::from_arg).collect(),
        }
    }

    fn into_remote(self) -> Result<RemoteResource, String> {
        let arguments: Vec<pp_client::ArgSpec> = self
            .arguments
            .into_iter()
            .map(ArgSpecView::into_arg)
            .collect();
        let value = serde_json::json!({
            "name": self.name,
            "url": self.url,
            "kind": self.kind,
            "dialect": self.dialect,
            "description": self.description,
            "update_interval_secs": self.update_interval_secs,
            "enabled": self.enabled,
            "argument_values": self.argument_values,
            "icon": self.icon,
            "arguments": arguments,
        });
        serde_json::from_value::<RemoteResource>(value).map_err(|e| e.to_string())
    }
}

/// 一次 `fetch_remotes` 拉取报告的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct FetchReportView {
    pub fetched: usize,
    pub scripts: usize,
    pub rewrites: usize,
    pub tasks: usize,
    pub warnings: Vec<String>,
}

/// 模块参数声明的对外视图（`#!arguments=` / Loon `[Argument]` 段）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArgSpecView {
    pub key: String,
    pub default_value: String,
    pub description: Option<String>,
    /// 控件类型：`Input`（文本输入）/ `Select`（下拉选择）。
    pub kind: String,
    /// `Select` 控件的可选项。
    pub options: Vec<String>,
    /// 参数分组标签（无分组时为 null）。
    pub tag: Option<String>,
}

impl ArgSpecView {
    /// 从内部 [`pp_client::ArgSpec`] 构造视图。
    fn from_arg(arg: &pp_client::ArgSpec) -> Self {
        Self {
            key: arg.key.clone(),
            default_value: arg.default_value.clone(),
            description: arg.description.clone(),
            kind: match arg.kind {
                pp_client::ArgKind::Select => "Select".to_string(),
                pp_client::ArgKind::Input => "Input".to_string(),
            },
            options: arg.options.clone(),
            tag: arg.tag.clone(),
        }
    }

    /// 转为内部 [`pp_client::ArgSpec`]（未知控件类型按 `Input` 处理）。
    fn into_arg(self) -> pp_client::ArgSpec {
        pp_client::ArgSpec {
            key: self.key,
            default_value: self.default_value,
            description: self.description,
            kind: match self.kind.as_str() {
                "Select" => pp_client::ArgKind::Select,
                _ => pp_client::ArgKind::Input,
            },
            options: self.options,
            tag: self.tag,
        }
    }
}

/// 配置头 `#!key=value` 元数据的对外视图。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigMetaView {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub author: Option<String>,
    pub icon: Option<String>,
    pub date: Option<String>,
    pub category: Option<String>,
    pub open_url: Option<String>,
    /// 模块参数声明（`#!arguments=` / `#!arguments-desc=`；无声明时为空列表）。
    pub arguments: Vec<ArgSpecView>,
}

impl ConfigMetaView {
    fn from_meta(meta: &pp_client::ConfigMeta) -> Self {
        Self {
            name: meta.name.clone(),
            desc: meta.desc.clone(),
            author: meta.author.clone(),
            icon: meta.icon.clone(),
            date: meta.date.clone(),
            category: meta.category.clone(),
            open_url: meta.open_url.clone(),
            arguments: meta
                .arguments
                .iter()
                .map(|arg| ArgSpecView {
                    key: arg.key.clone(),
                    default_value: arg.default_value.clone(),
                    description: arg.description.clone(),
                    kind: match arg.kind {
                        pp_client::ArgKind::Select => "Select".to_string(),
                        pp_client::ArgKind::Input => "Input".to_string(),
                    },
                    options: arg.options.clone(),
                    tag: arg.tag.clone(),
                })
                .collect(),
        }
    }
}

/// 一次配置导入摘要的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummaryView {
    pub rewrites: usize,
    pub scripts: usize,
    pub tasks: usize,
    pub hostnames: usize,
    pub warnings: Vec<String>,
    /// 配置头解析出的元数据（前端展示名称/描述等）。
    pub meta: ConfigMetaView,
}

/// 列出全部远程资源（`remotes.json` 清单）。
#[tauri::command]
pub async fn list_remotes(state: State<'_, AppState>) -> Result<Vec<RemoteResourceView>, String> {
    let manager = RemoteManager::new(state.data_dir.clone());
    let remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
    Ok(remotes
        .iter()
        .map(RemoteResourceView::from_remote)
        .collect())
}

/// 新增一条远程资源；重名时报错。
///
/// 添加时若携带参数声明（`arguments`，由前端从 detect meta 传入），后端按声明默认值
/// 预填 `argument_values`；URL 在落盘前经 [`pp_client::normalize_resource_url`] 归一化
/// （GitHub blob/raw → raw.githubusercontent.com）。
#[tauri::command]
pub async fn add_remote(
    state: State<'_, AppState>,
    remote: RemoteResourceView,
) -> Result<(), String> {
    if remote.name.trim().is_empty() {
        return Err("资源名不能为空".to_string());
    }
    if remote.url.trim().is_empty() {
        return Err("URL 不能为空".to_string());
    }
    let mut remote = remote.into_remote()?;
    remote.url = pp_client::normalize_resource_url(&remote.url);
    pp_client::prefill_argument_values(&mut remote);
    let manager = RemoteManager::new(state.data_dir.clone());
    let mut remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
    if remotes.iter().any(|r| r.name == remote.name) {
        return Err(format!("远程资源 '{}' 已存在", remote.name));
    }
    let name = remote.name.clone();
    let icon = remote.icon.clone();
    remotes.push(remote);
    manager
        .save(&remotes)
        .map_err(|e| format!("保存远程资源失败: {e}"))?;
    // 图标本地化缓存预热（best-effort）：失败仅记日志，不影响命令结果。
    if let Some(icon_url) = icon {
        if let Err(e) = manager.cache_icon(&name, &icon_url).await {
            tracing::warn!(name = %name, error = %e, "icon cache warmup failed");
        }
    }
    Ok(())
}

/// 删除一条远程资源；不存在时报错。
#[tauri::command]
pub async fn remove_remote(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let manager = RemoteManager::new(state.data_dir.clone());
    let mut remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
    let before = remotes.len();
    remotes.retain(|r| r.name != name);
    if remotes.len() == before {
        return Err(format!("远程资源 '{}' 不存在", name));
    }
    manager
        .save(&remotes)
        .map_err(|e| format!("保存远程资源失败: {e}"))
}

/// 按 name 定位全量更新一条远程资源（替代「删除重加」）；不存在时报错。
///
/// 与 [`add_remote`] 一致：URL 归一化、携带参数声明时按默认值预填 `argument_values`；
/// 既有缓存（`remote_cache/` / `scripts/`）保留。
#[tauri::command]
pub async fn update_remote(
    state: State<'_, AppState>,
    resource: RemoteResourceView,
) -> Result<(), String> {
    if resource.name.trim().is_empty() {
        return Err("资源名不能为空".to_string());
    }
    if resource.url.trim().is_empty() {
        return Err("URL 不能为空".to_string());
    }
    let mut remote = resource.into_remote()?;
    remote.url = pp_client::normalize_resource_url(&remote.url);
    pp_client::prefill_argument_values(&mut remote);
    let name = remote.name.clone();
    let icon = remote.icon.clone();
    let manager = RemoteManager::new(state.data_dir.clone());
    manager
        .update_resource(&name, remote)
        .map_err(|e| format!("更新远程资源失败: {e}"))?;
    // 图标本地化缓存预热（best-effort）：失败仅记日志，不影响命令结果。
    if let Some(icon_url) = icon {
        if let Err(e) = manager.cache_icon(&name, &icon_url).await {
            tracing::warn!(name = %name, error = %e, "icon cache warmup failed");
        }
    }
    Ok(())
}

/// 读取远程资源本地图标缓存，返回 `data:{mime};base64,...` data URL。
///
/// 缓存不存在（未下载 / 已删除）时返回 `None`，前端回退远程 URL / 首字母。
/// MIME 按扩展名推断：png/jpg/jpeg/webp/gif/svg/ico，未知按
/// `application/octet-stream`。
#[tauri::command]
pub async fn get_remote_icon(
    state: State<'_, AppState>,
    name: String,
) -> Result<Option<String>, String> {
    use base64::Engine as _;
    let manager = RemoteManager::new(state.data_dir.clone());
    let Some(path) = manager.icon_file(&name) else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("读取图标缓存失败: {e}"))?;
    let mime = match path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        _ => "application/octet-stream",
    };
    let encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(Some(format!("data:{mime};base64,{encoded}")))
}

/// 远程资源类型的字符串表示（与 `RemoteResourceView.kind` 的 serde 表示一致）。
fn remote_kind_str(kind: RemoteKind) -> &'static str {
    match kind {
        RemoteKind::Script => "Script",
        RemoteKind::Snippet => "Snippet",
    }
}

/// 脚本方言的字符串表示（与 `RemoteResourceView.dialect` 的 serde 表示一致）。
///
/// QX 已并入 Loon 生态（Loon 方言同时注入 QX 与 Surge 两套 API），detect
/// 嗅探出的 QuantumultX 统一映射为 `Loon`，前端不再暴露 QX 选项。
fn script_dialect_str(dialect: ScriptDialect) -> &'static str {
    match dialect {
        ScriptDialect::QuantumultX => "Loon",
        ScriptDialect::Surge => "Surge",
        ScriptDialect::Loon => "Loon",
    }
}

/// `detect_remote` 的嗅探结果：后缀判定的类型/方言 + Snippet 拉取解析出的配置头元数据。
#[derive(Debug, Clone, Serialize, Default)]
pub struct DetectRemoteView {
    /// 嗅探出的资源类型（`Script` / `Snippet`；无法识别时为 `None`）。
    pub kind: Option<String>,
    /// 嗅探出的脚本方言（`Surge` / `Loon`；无法识别时为 `None`）。
    pub dialect: Option<String>,
    /// 配置头元数据（仅 Snippet 且 URL 可访问时解析；拉取失败或非 Snippet 时为 `None`）。
    pub meta: Option<ConfigMetaView>,
}

/// 拉取单个 URL 的文本内容（15 秒超时）用于元数据嗅探；任何失败返回 `None`。
///
/// 委托 [`pp_client::fetch_resource_text`]：GitHub 代理前缀与「走本地代理」设置生效，
/// 拉取失败静默（detect 语义不变，meta 置 None）。
async fn fetch_detect_text(data_dir: &std::path::Path, url: &str) -> Option<String> {
    pp_client::fetch_resource_text(data_dir, url, std::time::Duration::from_secs(15))
        .await
        .ok()
}

/// 嗅探远端资源 URL：按后缀判定类型/方言；Snippet 且 URL 可访问时拉取内容解析配置头元数据。
///
/// GitHub blob/raw 链接在嗅探与拉取前归一化为 `raw.githubusercontent.com`（与
/// 拉取路径一致，避免网页端 blob 链接拉不到原始内容）。
/// 拉取失败不报错：后缀判定结果（`kind` / `dialect`）照常返回，`meta` 置 `None`，
/// 前端仅用返回字段预填添加表单。
#[tauri::command]
pub async fn detect_remote(
    state: State<'_, AppState>,
    url: String,
) -> Result<DetectRemoteView, String> {
    let url = pp_client::normalize_resource_url(url.trim());
    let (kind, dialect) = match detect_resource_from_url(&url) {
        Some((kind, dialect)) => (
            Some(remote_kind_str(kind).to_string()),
            Some(script_dialect_str(dialect).to_string()),
        ),
        None => (None, None),
    };

    // 仅 Snippet 且 URL 可访问时拉取内容解析元数据；失败静默（meta 置 None）。
    let meta = if kind.as_deref() == Some("Snippet")
        && (url.starts_with("http://") || url.starts_with("https://"))
    {
        fetch_detect_text(&state.data_dir, &url)
            .await
            .map(|text| {
                let parsed = parse_config_meta(&text);
                if parsed != ConfigMeta::default() {
                    Some(ConfigMetaView::from_meta(&parsed))
                } else {
                    None
                }
            })
            .unwrap_or(None)
    } else {
        None
    };

    Ok(DetectRemoteView {
        kind,
        dialect,
        meta,
    })
}

/// 拉取全部启用的远程资源（无系统代理，30 秒超时）。
#[tauri::command]
pub async fn fetch_remotes(state: State<'_, AppState>) -> Result<FetchReportView, String> {
    let manager = RemoteManager::new(state.data_dir.clone());
    let remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
    let report = manager.fetch_all(&remotes).await;
    Ok(FetchReportView {
        fetched: report.fetched,
        scripts: report.scripts,
        rewrites: report.rewrites,
        tasks: report.tasks,
        warnings: report.warnings,
    })
}

/// 探测 GitHub 访问链路可用性：按真实拉取管线（GitHub 代理前缀 / 走本地代理）
/// 请求轻量 GitHub URL，返回耗时毫秒；失败上抛错误信息。
#[tauri::command]
pub async fn test_github_proxy(state: State<'_, AppState>) -> Result<String, String> {
    // 轻量、纯文本：用于链路连通性探测。
    let url = "https://api.github.com/zen";
    let started = std::time::Instant::now();
    pp_client::fetch_resource_text(&state.data_dir, url, std::time::Duration::from_secs(10))
        .await
        .map_err(|e| e.to_string())?;
    let elapsed_ms = started.elapsed().as_millis();
    Ok(format!("OK（{elapsed_ms} ms）"))
}

/// 列出定时任务；客户端未启动或调度器未就绪时返回空列表。
#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskScriptView>, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(Vec::new());
    };
    let Some(scheduler) = client.scheduler() else {
        return Ok(Vec::new());
    };
    Ok(scheduler.list_tasks())
}

/// 手动运行一个定时任务；返回脚本 `$done` 输出的 JSON 字符串。
///
/// 脚本执行由 pp-script 的 `ScriptWorker` 在专有线程驱动（`Send` future），
/// `scheduler.run_now` 的 future 亦为 `Send`，可直接在 Tauri 命令中 `await`。
#[tauri::command]
pub async fn run_task(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let scheduler = {
        let lock = state.client.lock().await;
        let client = lock
            .as_ref()
            .ok_or_else(|| "客户端未启动，无法运行任务".to_string())?;
        client
            .scheduler_handle()
            .ok_or_else(|| "任务调度器未就绪".to_string())?
    };
    let output = scheduler
        .run_now(&name)
        .await
        .map_err(|e| format!("运行任务失败: {e}"))?;
    Ok(output.0.to_string())
}

/// 导入 Surge / Loon 配置片段：解析 → 拉取脚本源码回填 → 合并写入本地导入缓存
/// `remote_cache/imported.json`。单个脚本拉取失败记 warning 跳过，不阻塞其他规则合入。
#[tauri::command]
pub async fn import_config(
    state: State<'_, AppState>,
    content: String,
    dialect: String,
) -> Result<ImportSummaryView, String> {
    let script_dialect = match dialect.as_str() {
        // QX 已并入 Loon 生态（Loon 方言同时注入 QX 与 Surge 两套 API）；
        // 保留 quantumultx 输入兼容旧前端，语义归入 Loon。
        "surge" => ScriptDialect::Surge,
        "loon" | "quantumultx" => ScriptDialect::Loon,
        other => {
            return Err(format!("未知方言 '{other}'（可选: surge / loon）"));
        }
    };
    let manager = RemoteManager::new(state.data_dir.clone());
    let summary = manager
        .import_content(&content, script_dialect)
        .await
        .map_err(|e| format!("导入配置失败: {e}"))?;
    Ok(ImportSummaryView {
        rewrites: summary.rewrites,
        scripts: summary.scripts,
        tasks: summary.tasks,
        hostnames: summary.hostnames,
        warnings: summary.warnings,
        meta: ConfigMetaView::from_meta(&summary.meta),
    })
}

/// 核心类型序列化为前端小写约定（`singbox` / `mihomo`）。
fn core_type_str(core_type: CoreType) -> String {
    serde_json::to_value(core_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// 解析前端小写核心类型字符串（`singbox` / `mihomo`）。
fn core_type_from_str(s: &str) -> Result<CoreType, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("无效的核心类型 '{s}'（可选: singbox / mihomo）"))
}

/// 解析模板 ID 字符串为 `Uuid`。
fn parse_profile_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("无效的模板 ID: {e}"))
}

/// 复写模板列表视图（与前端 `ProfileView` TS 类型对齐）。
#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    /// 核心类型：`singbox` / `mihomo`（与 `pp_common::CoreType` 的 serde 表示一致）。
    pub core_type: String,
    /// YAML 复写字节数（列表展示用）。
    pub yaml_bytes: u64,
    /// JS 复写字节数（列表展示用）。
    pub js_bytes: u64,
    /// 远程 YAML 复写 URL（`null` = 未配置）。
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（`null` = 未配置）。
    pub js_url: Option<String>,
}

impl ProfileView {
    fn from_profile(p: &Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            core_type: core_type_str(p.core_type),
            yaml_bytes: p.yaml_override.len() as u64,
            js_bytes: p.js_override.len() as u64,
            yaml_url: p.yaml_url.clone(),
            js_url: p.js_url.clone(),
        }
    }
}

/// 复写模板详情视图（含完整复写内容，与前端 `ProfileDetailView` TS 类型对齐）。
#[derive(Debug, Clone, Serialize)]
pub struct ProfileDetailView {
    pub id: String,
    pub name: String,
    pub core_type: String,
    /// YAML 深合并复写（空串 = 未启用）。
    pub yaml_override: String,
    /// JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。
    pub js_override: String,
    /// 远程 YAML 复写 URL（`null` = 未配置）。
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（`null` = 未配置）。
    pub js_url: Option<String>,
}

impl ProfileDetailView {
    fn from_profile(p: &Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            core_type: core_type_str(p.core_type),
            yaml_override: p.yaml_override.clone(),
            js_override: p.js_override.clone(),
            yaml_url: p.yaml_url.clone(),
            js_url: p.js_url.clone(),
        }
    }
}

/// 列出全部复写模板（`data_dir/profiles.json`）。
#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileView>, String> {
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    Ok(profiles.iter().map(ProfileView::from_profile).collect())
}

/// 新建复写模板的入参。
#[derive(Debug, Deserialize)]
pub struct CreateProfileInput {
    pub name: String,
    /// 核心类型：`singbox` / `mihomo`。
    pub core_type: String,
}

/// 新建复写模板；重名时报错（错误信息由存储层上抛）。
#[tauri::command]
pub fn create_profile(
    state: State<'_, AppState>,
    input: CreateProfileInput,
) -> Result<ProfileView, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("模板名称不能为空".to_string());
    }
    let core_type = core_type_from_str(&input.core_type)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profile = store
        .add(&name, core_type)
        .map_err(|e| format!("创建模板失败: {e}"))?;
    Ok(ProfileView::from_profile(&profile))
}

/// 读取单个复写模板详情（含 YAML / JS 复写内容）；模板不存在时报错。
#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, id: String) -> Result<ProfileDetailView, String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    let profile = profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "模板不存在".to_string())?;
    Ok(ProfileDetailView::from_profile(profile))
}

/// 更新复写模板的入参（`core_type` 保持存储值，运行时按订阅关联取用模板）。
#[derive(Debug, Deserialize)]
pub struct UpdateProfileInput {
    pub id: String,
    pub name: String,
    pub yaml_override: String,
    pub js_override: String,
    /// 远程 YAML 复写 URL（可选；http/https，空串 = 未配置）。
    #[serde(default)]
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（可选；http/https，空串 = 未配置）。
    #[serde(default)]
    pub js_url: Option<String>,
}

/// 校验 YAML 复写：非空时必须是可解析的 YAML mapping（与 `apply_yaml_override` 一致）。
fn validate_yaml_override(yaml: &str) -> Result<(), String> {
    if yaml.trim().is_empty() {
        return Ok(());
    }
    let patch: serde_json::Value =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML 复写解析失败: {e}"))?;
    if patch.is_null() {
        return Ok(());
    }
    if !patch.is_object() {
        return Err("YAML 复写必须是 mapping（对象）".to_string());
    }
    Ok(())
}

/// 粗检 JS 复写是否定义了 `main` 函数（同步纯函数模式）。
fn validate_js_override(js: &str) -> Result<(), String> {
    if js.trim().is_empty() {
        return Ok(());
    }
    if !js.contains("function main") && !js.contains("main(") {
        return Err(
            "JS 复写需定义 main 函数（function main(config) { ... return config; }）".to_string(),
        );
    }
    Ok(())
}

/// 校验远程复写 URL：`Some` 且非空时必须是 http:// 或 https:// 开头。
fn validate_remote_url(url: &Option<String>) -> Result<(), String> {
    if let Some(url) = url {
        let url = url.trim();
        if !(url.is_empty() || url.starts_with("http://") || url.starts_with("https://")) {
            return Err("远程复写 URL 必须是 http:// 或 https:// 开头".to_string());
        }
    }
    Ok(())
}

/// 空白串规范化为 `None`（空串 = 未配置）。
fn normalize_optional_url(url: Option<String>) -> Option<String> {
    url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
}

/// 更新复写模板的可编辑字段（name / yaml_override / js_override / yaml_url /
/// js_url）；校验失败不落盘。
#[tauri::command]
pub fn update_profile(state: State<'_, AppState>, input: UpdateProfileInput) -> Result<(), String> {
    let id = parse_profile_id(&input.id)?;
    validate_yaml_override(&input.yaml_override)?;
    validate_js_override(&input.js_override)?;
    validate_remote_url(&input.yaml_url)?;
    validate_remote_url(&input.js_url)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let mut profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    let target = profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| "模板不存在".to_string())?;
    target.name = input.name;
    target.yaml_override = input.yaml_override;
    target.js_override = input.js_override;
    target.yaml_url = normalize_optional_url(input.yaml_url);
    target.js_url = normalize_optional_url(input.js_url);
    store
        .save(&profiles)
        .map_err(|e| format!("保存复写模板失败: {e}"))
}

/// 删除复写模板；不存在时报错。
#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    store
        .remove(id)
        .map_err(|e| format!("删除复写模板失败: {e}"))
}

/// 生成生效配置预览：拉取订阅 → 内置模板 → 订阅关联模板的远程 + 本地复写叠加 → 核心合成（不含 MITM 链路）。
///
/// 返回最终核心可用的配置文本（sing-box 为 JSON、mihomo 为 YAML），供只读预览。
/// 需要已保存的客户端配置（`data_dir/client.json`）。订阅选择与覆写解析与启动路径
/// （`state.rs`）一致：
/// - `subscription_id = Some(id)`：按指定订阅预览（忽略 enabled 状态，订阅表格
///   行内「预览」使用），不存在报错「订阅不存在」。
/// - `subscription_id = None`：首页选中的订阅（`active_subscription_id`）唯一生效，
///   其关联的覆写模板经 `resolve_remote_overrides` 拉取/缓存回退叠加（缓存目录
///   `data_dir/profile_cache`），与本地复写一起由 `build_core_config_v2` 合成；
///   未选中订阅时回退旧版 Hub 订阅路径（deprecated，无覆写）。
#[tauri::command]
pub async fn preview_core_config(
    state: State<'_, AppState>,
    subscription_id: Option<String>,
) -> Result<String, String> {
    // 空串 / `None` 均视为「未指定」，沿用首页选中订阅路径。
    let preview_id = match subscription_id.as_deref() {
        Some(s) if !s.trim().is_empty() => Some(parse_subscription_id(s.trim())?),
        _ => None,
    };
    preview_core_config_impl(state.data_dir.clone(), preview_id).await
}

/// 预览的具体实现（`build_core_config_v2` 的 JS 复写经 pp-script `ScriptWorker`
/// 驱动，future 为 `Send`，可直接在 Tauri 命令中 await）。
///
/// `preview_id = Some(id)` 时按指定订阅预览（不存在报错「订阅不存在」，忽略
/// enabled 状态）；`None` 时保持既有 `active_subscription_id` 选择逻辑（含停用
/// 校验与旧版 Hub 回退）。
async fn preview_core_config_impl(
    data_dir: PathBuf,
    preview_id: Option<Uuid>,
) -> Result<String, String> {
    let cfg = ClientConfig::load(&data_dir)
        .map_err(|e| format!("未找到已保存的配置（{e}），请先在设置页保存配置"))?;
    // 远程复写缓存目录（与启动路径一致：`data_dir/profile_cache`）。
    let cache_dir = data_dir.join("profile_cache");

    // 订阅选择：指定 id 时定位订阅（忽略 enabled）；未指定时沿用首页选中订阅
    // （`active_subscription_id`），未选中回退旧版 Hub 订阅路径（deprecated，无覆写）。
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
        let fetch = fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
            .await
            .map_err(|e| format!("拉取订阅「{}」失败: {e}", sub.name))?;
        // 格式兼容校验（等价启动路径 `state.rs` 的 `check_subscription_core_compat`）：
        // 预览任意订阅时，订阅格式与当前核心不兼容直接报错（指明订阅名）。
        check_preview_core_compat(fetch.format, cfg.core_type)
            .map_err(|e| format!("订阅「{}」无法预览: {e}", sub.name))?;
        match cfg.core_type {
            CoreType::SingBox => {
                SubContent::SingBox(serde_json::json!({ "outbounds": fetch.singbox_nodes }))
            }
            CoreType::Mihomo => {
                let yaml = serde_yaml::to_string(&serde_json::json!({
                    "proxies": fetch.mihomo_nodes,
                }))
                .map_err(|e| format!("序列化配置失败: {e}"))?;
                SubContent::Mihomo(yaml)
            }
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
                let fetch = fetch_subscription_with_ua(&sub.url, sub.user_agent.as_deref())
                    .await
                    .map_err(|e| format!("拉取订阅失败: {e}"))?;
                match cfg.core_type {
                    CoreType::SingBox => {
                        SubContent::SingBox(serde_json::json!({ "outbounds": fetch.singbox_nodes }))
                    }
                    CoreType::Mihomo => {
                        let yaml = serde_yaml::to_string(&serde_json::json!({
                            "proxies": fetch.mihomo_nodes,
                        }))
                        .map_err(|e| format!("序列化配置失败: {e}"))?;
                        SubContent::Mihomo(yaml)
                    }
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

    // 覆写解析（纯关联制，与启动路径一致）：当前生效订阅关联的覆写模板；
    // 订阅未关联（或 legacy Hub 回退路径）不使用任何覆写。指定订阅预览时
    // 错误信息指明订阅名。
    let sub_name = specified.as_ref().map(|s| s.name.as_str());
    let store = ProfileStoreV2::new(data_dir);
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

    // 设置页最高优先级的 TUN / Clash 面板配置：compose 之后强制注入，预览与实际
    // 启动路径（state.rs）一致。
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

/// 订阅格式 ↔ 核心类型绑定校验（等价启动路径 `state.rs` 的
/// `check_subscription_core_compat`）：
///
/// - [`SubFormat::SingBoxJson`] 仅支持 sing-box 核心（其节点为 sing-box JSON，跨格式
///   转 mihomo 有信息丢失）；
/// - [`SubFormat::ClashYaml`] 仅支持 mihomo 核心（clash 订阅节点转 sing-box outbound
///   时丢失 TLS 块导致 sing-box `TLS required` FATAL）；
/// - [`SubFormat::ShareLinks`] 双核心皆可。
///
/// 不匹配时返回明确错误（含检测到的订阅格式与当前核心类型）。
fn check_preview_core_compat(format: SubFormat, core_type: CoreType) -> Result<(), String> {
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

// ---------- 核心版本管理 ----------

/// 本地核心的对外视图（与前端 `LocalCoreView` TS 类型对齐）。
#[derive(Debug, Clone, Serialize)]
pub struct LocalCoreView {
    /// 核心类型：`singbox` / `mihomo`。
    pub core_type: String,
    pub version: String,
    pub path: String,
    /// 来源：`downloaded`（已下载）/ `system`（系统探测）。
    pub source: String,
    /// 是否为当前启用的核心（`core_binary` 匹配）。
    pub active: bool,
}

impl LocalCoreView {
    fn from_core(core: &pp_client::LocalCore, active_binary: &std::path::Path) -> Self {
        Self {
            core_type: core_type_str(core.core_type),
            version: core.version.clone(),
            path: core.path.to_string_lossy().into_owned(),
            source: match core.source {
                pp_client::CoreSource::Downloaded => "downloaded",
                pp_client::CoreSource::System => "system",
            }
            .to_string(),
            active: core.path == active_binary,
        }
    }
}

/// 合并已下载与系统探测列表（按路径去重，系统探测优先展示）。
fn merge_cores(
    mut installed: Vec<pp_client::LocalCore>,
    system: Vec<pp_client::LocalCore>,
) -> Vec<pp_client::LocalCore> {
    for s in system {
        if !installed.iter().any(|c| c.path == s.path) {
            installed.push(s);
        }
    }
    installed
}

/// 当前配置中的 active 核心二进制路径（配置未保存时为空）。
fn active_binary(data_dir: &std::path::Path) -> std::path::PathBuf {
    pp_client::ClientConfig::load(data_dir)
        .map(|c| c.core_binary)
        .unwrap_or_default()
}

/// 列出本地可用核心（已下载 + 系统探测，含 active 标记）。
#[tauri::command]
pub async fn list_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let cores = merge_cores(inv.list_installed(), inv.detect_system_cores());
    let active = active_binary(&state.data_dir);
    Ok(cores
        .iter()
        .map(|c| LocalCoreView::from_core(c, &active))
        .collect())
}

/// 列出远端最近 10 个发布版本（GitHub releases，去 `v` 前缀）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_remote_core_versions(
    state: State<'_, AppState>,
    core_type: String,
) -> Result<Vec<String>, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    inv.list_remote_versions(ct)
        .await
        .map_err(|e| format!("拉取远端版本失败: {e}"))
}

/// 列出指定核心类型已下载的版本（版本目录扫描，语义化版本倒序）的具体实现。
fn list_downloaded_versions_impl(
    data_dir: &std::path::Path,
    core_type: String,
) -> Result<Vec<String>, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(data_dir.to_path_buf());
    Ok(inv.list_downloaded_versions(ct))
}

/// 列出指定核心类型已下载的版本（版本目录扫描，语义化版本倒序）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_downloaded_versions(
    state: State<'_, AppState>,
    core_type: String,
) -> Result<Vec<String>, String> {
    list_downloaded_versions_impl(&state.data_dir, core_type)
}

/// 下载指定版本核心并返回其视图。
#[tauri::command(rename_all = "snake_case")]
pub async fn download_core(
    state: State<'_, AppState>,
    core_type: String,
    version: String,
) -> Result<LocalCoreView, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let core = inv
        .download(ct, &version)
        .await
        .map_err(|e| format!("下载核心失败: {e}"))?;
    let active = active_binary(&state.data_dir);
    Ok(LocalCoreView::from_core(&core, &active))
}

/// 将指定路径设为核心二进制：校验存在且可执行后，先在本地核心清单
/// （已下载 + 系统探测合并）中按路径匹配其 `core_type`，未命中则按文件名
/// 推断；随后同时写回 `client.json` 的 `core_binary` 与 `core_type`，避免
/// 启用异类型核心时两者不一致。无法识别类型时返回错误提示手动选择。
#[tauri::command(rename_all = "snake_case")]
pub async fn set_active_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let bin = PathBuf::from(&path);
    if !bin.is_file() {
        return Err(format!("核心二进制不存在: {path}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&bin).map_err(|e| format!("读取核心信息失败: {e}"))?;
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(format!("核心二进制不可执行: {path}"));
        }
    }
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let core_type = merge_cores(inv.list_installed(), inv.detect_system_cores())
        .into_iter()
        .find(|c| c.path == bin)
        .map(|c| c.core_type)
        .or_else(|| pp_client::infer_core_type(&bin))
        .ok_or_else(|| format!("无法识别核心类型: {path}，请在设置页手动选择核心类型"))?;
    let mut config = match pp_client::ClientConfig::load(&state.data_dir) {
        Ok(cfg) => cfg,
        Err(_) => pp_client::ClientConfig::new(
            state.data_dir.clone(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        ),
    };
    config.core_binary = bin;
    config.core_type = core_type;
    config.save().map_err(|e| format!("保存配置失败: {e}"))
}

/// 手动刷新系统核心探测。
#[tauri::command]
pub async fn detect_system_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let active = active_binary(&state.data_dir);
    Ok(inv
        .detect_system_cores()
        .iter()
        .map(|c| LocalCoreView::from_core(c, &active))
        .collect())
}

/// `delete_core` 的具体实现（命令层可测试的纯逻辑）。
///
/// 删除约束（与前端设置页核心管理一致）：
/// - 系统来源核心不可删除（合并清单中该 path 来源为 `System`）；
/// - 当前使用中的核心不可删除（`path == active_binary(data_dir)`）；
/// - 其余委托 [`pp_client::ClientCoreInventory::delete`]（下载目录归属校验等）。
fn delete_core_impl(data_dir: &std::path::Path, path: &str) -> Result<(), String> {
    let bin = PathBuf::from(path);
    let inv = pp_client::ClientCoreInventory::new(data_dir.to_path_buf());
    // 先拒绝系统来源核心（已下载 + 系统探测合并清单中命中且为 system）。
    let matched = merge_cores(inv.list_installed(), inv.detect_system_cores())
        .into_iter()
        .find(|c| c.path == bin);
    if matched.is_some_and(|c| c.source == pp_client::CoreSource::System) {
        return Err("系统核心不可删除：仅支持删除已下载的核心".to_string());
    }
    // 再拒绝正在使用的核心。
    let active = active_binary(data_dir);
    if bin == active {
        return Err("正在使用的核心不可删除：请先切换其他核心".to_string());
    }
    inv.delete(&bin, &active)
        .map_err(|e| format!("删除核心失败: {e}"))
}

/// 删除一个已下载核心（系统来源 / 当前使用中的核心不可删除）。
#[tauri::command(rename_all = "snake_case")]
pub async fn delete_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    delete_core_impl(&state.data_dir, &path)
}

// ---------- 订阅管理 ----------

/// 订阅用户信息的对外视图（与 `pp_client::SubscriptionInfo` 字段对齐）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionUserInfoView {
    /// 已用上行字节数。
    pub upload: Option<u64>,
    /// 已用下行字节数。
    pub download: Option<u64>,
    /// 总流量字节数。
    pub total: Option<u64>,
    /// 到期时间戳（秒）。
    pub expire: Option<u64>,
}

impl SubscriptionUserInfoView {
    fn from_info(info: &pp_client::SubscriptionInfo) -> Self {
        Self {
            upload: info.upload,
            download: info.download,
            total: info.total,
            expire: info.expire,
        }
    }
}

/// 一条订阅的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    /// 关联的覆写模板 id（`data_dir/profiles.json` 中的模板 id；`null` = 不使用覆写）。
    pub profile_id: Option<String>,
    pub userinfo: Option<SubscriptionUserInfoView>,
    /// 最近一次 fetch 成功的节点数（sing-box 侧可用节点数）。
    pub node_count: u64,
    /// 最近一次 fetch 的错误信息（失败时记录；不阻塞已有数据展示）。
    pub error: Option<String>,
    /// 最近一次 fetch 嗅探出的订阅内容格式（`ShareLinks` / `ClashYaml` /
    /// `SingBoxJson`）；未成功拉取时为 `None`。
    pub format: Option<String>,
    /// 拉取时使用的请求 User-Agent（`None` / 空串 = 默认 clash.meta）。
    pub user_agent: Option<String>,
}

impl SubscriptionView {
    fn from_sub(sub: &Subscription) -> Self {
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

/// [`SubFormat`] 的字符串表示（与前端 `SubscriptionFormat` 的联合类型对齐）。
fn sub_format_str(format: SubFormat) -> &'static str {
    match format {
        SubFormat::ShareLinks => "ShareLinks",
        SubFormat::ClashYaml => "ClashYaml",
        SubFormat::SingBoxJson => "SingBoxJson",
    }
}

/// 添加订阅的入参。
#[derive(Debug, Deserialize)]
pub struct AddSubscriptionInput {
    pub name: String,
    pub url: String,
    /// 请求 User-Agent；`None` / 空串使用默认 `clash.meta`。
    pub user_agent: Option<String>,
    /// 关联的覆写模板 id（Uuid 字符串；`None` / 空串 = 不使用覆写）。
    #[serde(default)]
    pub profile_id: Option<String>,
}

/// 校验订阅 URL 必须为 http/https。
fn validate_subscription_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err("订阅 URL 必须以 http:// 或 https:// 开头".to_string())
    }
}

/// 解析订阅 ID 字符串为 `Uuid`。
fn parse_subscription_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("无效的订阅 ID: {e}"))
}

/// 解析订阅关联的覆写模板 ID：`None` / 空串 = 未关联（不使用覆写）；非空必须是合法 Uuid。
fn parse_profile_ref(profile_id: &Option<String>) -> Result<Option<Uuid>, String> {
    match profile_id.as_deref() {
        Some(s) if !s.trim().is_empty() => Ok(Some(parse_profile_id(s.trim())?)),
        _ => Ok(None),
    }
}

/// 把一次 fetch 结果合并进订阅（成功更新 userinfo / 节点数并清空 error；
/// 失败仅记录 error，保留旧数据）。拉取 UA 取订阅条目配置（`None` → 默认 clash.meta）。
async fn apply_fetch(sub: &mut Subscription, url: &str) {
    match fetch_subscription_with_ua(url, sub.user_agent.as_deref()).await {
        Ok(result) => {
            sub.userinfo = result.userinfo;
            sub.node_count = result.singbox_nodes.len() as u64;
            sub.format = Some(result.format);
            sub.error = None;
        }
        Err(e) => {
            sub.error = Some(format!("拉取失败: {e}"));
        }
    }
}

/// 将更新后的订阅按 id 写回存储。
fn write_subscription(store: &SubscriptionStore, sub: &Subscription) -> Result<(), String> {
    let mut subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    if let Some(existing) = subs.iter_mut().find(|s| s.id == sub.id) {
        *existing = sub.clone();
    }
    store.save(&subs).map_err(|e| format!("保存订阅失败: {e}"))
}

/// 列出全部订阅（含最近一次 fetch 的节点数与错误信息）。
#[tauri::command]
pub async fn list_subscriptions(
    state: State<'_, AppState>,
) -> Result<Vec<SubscriptionView>, String> {
    let store = SubscriptionStore::new(state.data_dir.clone());
    let subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    Ok(subs.iter().map(SubscriptionView::from_sub).collect())
}

/// 添加订阅：校验 URL → 落盘（默认启用）→ 写入关联的覆写模板 → 立即 fetch 一次
/// 拿 userinfo + 节点数。
///
/// fetch 失败不阻塞添加，错误记入返回视图的 `error` 字段。
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
    validate_subscription_url(&url)?;
    // 空串 / 纯空白 UA 归一化为 None（使用默认 clash.meta）。
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
    apply_fetch(&mut sub, &url).await;
    write_subscription(&store, &sub)?;
    Ok(SubscriptionView::from_sub(&sub))
}

/// 刷新订阅：重新 fetch 更新 userinfo / 节点数。
///
/// 失败时保留旧数据并返回（视图 `error` 字段携带错误信息）。
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
    apply_fetch(&mut subs[idx], &url).await;
    store
        .save(&subs)
        .map_err(|e| format!("保存订阅失败: {e}"))?;
    Ok(SubscriptionView::from_sub(&subs[idx]))
}

/// 删除订阅；不存在时静默返回。
#[tauri::command]
pub async fn remove_subscription(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_subscription_id(&id)?;
    let store = SubscriptionStore::new(state.data_dir.clone());
    store.remove(id).map_err(|e| format!("删除订阅失败: {e}"))
}

/// 切换订阅启用状态（`enabled` 表示「可被首页选择」；首页选中的订阅唯一生效）。
///
/// 停用当前选中的订阅（`client.json` 的 `active_subscription_id`）时自动清除选中。
#[tauri::command]
pub async fn set_subscription_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let id = parse_subscription_id(&id)?;
    set_subscription_enabled_impl(&state.data_dir, id, enabled)
}

/// `set_subscription_enabled` 的具体实现（命令层可测试的纯逻辑）。
fn set_subscription_enabled_impl(
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

/// 设置首页选中的生效订阅（写入 `client.json` 的 `active_subscription_id`）。
///
/// `Some(id)`：校验订阅存在且 `enabled`（可被首页选择），不满足报错；`None`：清除选中。
#[tauri::command]
pub async fn set_active_subscription(
    state: State<'_, AppState>,
    id: Option<String>,
) -> Result<(), String> {
    set_active_subscription_impl(&state.data_dir, id)
}

/// `set_active_subscription` 的具体实现（命令层可测试的纯逻辑）。
fn set_active_subscription_impl(
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

/// 更新订阅的 name / url / user_agent / 关联覆写模板（替代「删除重加」）；订阅不存在时报错。
///
/// URL 变更时清空上次 fetch 的缓存（userinfo / 节点数）；URL 未变保留。URL 在落盘前
/// 经 [`pp_client::normalize_resource_url`] 归一化（GitHub blob/raw → raw）。
/// `profile_id` 语义：`Some(非空)` = 关联该覆写模板，`Some("")` / `None` = 取消关联
/// （前端总是传当前表单值）。
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
    validate_subscription_url(&url)?;
    // 空串 / 纯空白 UA 归一化为 None（使用默认 clash.meta）。
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

// ---------- 渲染能力检测 ----------

/// 是否具备 GPU 加速渲染能力（决定前端使用 HeroUI 动画 toast 还是自定义静态
/// toast）。
///
/// 检测规则：
/// - 非 Linux（Windows / macOS）→ `true`（无 WebKitGTK 软渲染问题）；
/// - Linux 且为 WSL（[`crate::is_wsl`]）：`/dev/dxg` 存在（WSLg GPU 直通）且
///   `LIBGL_ALWAYS_SOFTWARE` 未设置或为 `"0"` → `true`，否则 `false`；
/// - 原生 Linux：`LIBGL_ALWAYS_SOFTWARE=1` 时 `false`，否则 `true`（无法精确
///   探测时保守认为有 GPU）。
#[tauri::command]
pub fn gpu_acceleration() -> bool {
    let os_release = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok();
    let libgl_always_software = std::env::var("LIBGL_ALWAYS_SOFTWARE").ok();
    gpu_acceleration_impl(
        os_release.as_deref(),
        std::path::Path::new("/dev/dxg").exists(),
        libgl_always_software.as_deref(),
    )
}

/// `gpu_acceleration` 的纯判定逻辑（按注入的 `(os_release 内容, dxg 存在性,
/// env)` 参数化，便于命令层测试；平台是否为 Linux 由 `cfg!` 编译期决定）。
fn gpu_acceleration_impl(
    os_release: Option<&str>,
    has_dxg: bool,
    libgl_always_software: Option<&str>,
) -> bool {
    // 非 Linux（Windows / macOS）无 WSL 软渲染问题 → 有 GPU。
    if !cfg!(target_os = "linux") {
        return true;
    }
    // WSL：无 GPU 直通或强制软渲染时判定为无 GPU（前端用自定义静态 toast 规避
    // WebKitGTK view-transition 崩溃）。
    if os_release.is_some_and(crate::is_wsl_osrelease) {
        return has_dxg && libgl_always_software.is_none_or(|v| v == "0");
    }
    // 原生 Linux：仅显式强制软渲染时判定为无 GPU，否则保守认为有 GPU。
    libgl_always_software.is_none_or(|v| v != "1")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// 简易临时目录（测试用，避免新增 tempfile dev-dependency）：
    /// 进程 id + 原子计数器生成唯一路径，Drop 时递归清理。
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

    /// 全局 PATH 锁：环境变量是进程级状态，并行测试间互斥，避免相互串台。
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 在受控 PATH 下执行闭包（互斥串行化；避免宿主环境 PATH 中的真实核心干扰
    /// 系统探测回退）。
    fn with_empty_path<T>(f: impl FnOnce() -> T) -> T {
        with_patched_path(std::path::Path::new("/nonexistent-pp-test-bin"), f)
    }

    /// 把 PATH 替换为指定目录执行闭包（互斥串行化；供注入假系统核心测试用）。
    fn with_patched_path<T>(path: &std::path::Path, f: impl FnOnce() -> T) -> T {
        let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("PATH");
        // Rust 2024 下 std::env 的 set_var 标记为 unsafe（并发修改环境变量是未定义
        // 行为），PATH_LOCK 保证测试进程内串行访问。
        unsafe {
            std::env::set_var("PATH", path);
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

    /// 写入一个「已下载核心」假二进制（`cores/<dir>/<version>/<dir>`）。
    fn write_core(data_dir: &std::path::Path, core_dir: &str, version: &str) {
        let bin = data_dir
            .join("cores")
            .join(core_dir)
            .join(version)
            .join(core_dir);
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"fake core").unwrap();
    }

    // ---------- 项 1 回归：命令层序列化往返 ----------

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
        assert!(
            result.warning.is_none(),
            "完整 payload 不应有 warning: {:?}",
            result.warning
        );

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

        // load → from_config 往返保真（前端下次保存时的 payload 基础）。
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
        // 旧前端 / 部分 payload：缺失 system_proxy_enabled 等布尔字段，且 hub_url /
        // sub_token 为空 —— 缺字段按默认值补齐，空 hub 字段不再产生 warning。
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
        assert_eq!(view.clash_api_ui, "zashboard", "缺失字段按默认值补齐");
        assert!(
            view.github_proxy_prefix.is_empty() && !view.fetch_via_local_proxy,
            "旧前端缺失 GitHub 访问字段按默认值补齐"
        );
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 12345);
        assert!(!saved.mitm_enabled);
        assert!(!saved.system_proxy_enabled, "缺失布尔字段按默认值补齐");
        assert_eq!(saved.clash_api_ui, "zashboard");

        // 空 hub_url/sub_token 已退役：不再产生订阅缺失提示（core_type 联动警告
        // 属于另一独立路径，与本回归无关）。
        if let Some(w) = &result.warning {
            assert!(
                !w.contains("hub_url") && !w.contains("sub_token"),
                "空 hub_url/sub_token 不应再产生 warning: {w}"
            );
        }
    }

    #[test]
    fn save_config_empty_hub_and_token_saves_without_warning() {
        let dir = TestDir::new();
        let view = full_view(dir.path());

        // 先保存一次（含 hub_url/sub_token），随后用户清空两者保存基本设置改动。
        with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());
        let mut cleared = view;
        cleared.hub_url = String::new();
        cleared.sub_token = String::new();
        cleared.mixed_port = 30000;
        let result = with_empty_path(|| save_config_impl(dir.path(), cleared).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 30000, "基本设置应保存成功");
        assert!(
            result.warning.is_none(),
            "空 hub_url/sub_token 不应再提示: {:?}",
            result.warning
        );
    }

    // ---------- 项 2 回归：core_type 联动本地核心 ----------

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

        // 用户把 core_type 切到 mihomo，但 core_binary 仍指向 sing-box。
        let mut view = full_view(dir.path());
        view.core_type = "mihomo".to_string();
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::Mihomo);
        // 自动联动：填入已下载 mihomo 首选二进制。
        assert_eq!(
            saved.core_binary,
            dir.path().join("cores/mihomo/1.19.29/mihomo")
        );
        assert!(
            result.warning.is_none(),
            "找到本地核心时不应有联动 warning: {:?}",
            result.warning
        );
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
        // 找不到 mihomo 本地核心 → 保留原二进制并返回 warning。
        assert_eq!(
            saved.core_binary,
            dir.path().join("cores/sing-box/1.13.15/sing-box")
        );
        let warning = result.warning.expect("应返回联动 warning");
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
        // 未切换 core_type → 二进制保持原样（不触发联动）。
        assert_eq!(
            saved.core_binary,
            dir.path().join("cores/sing-box/1.13.15/sing-box")
        );
    }

    // ---------- 项 2：list_downloaded_versions 命令层 ----------

    #[test]
    fn list_downloaded_versions_lists_semantic_descending() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "sing-box", "1.14.0-beta.4");
        write_core(dir.path(), "sing-box", "1.14.0");
        write_core(dir.path(), "mihomo", "1.19.29");

        // 语义化倒序：1.14.0 > 1.14.0-beta.4 > 1.13.15。
        let versions = list_downloaded_versions_impl(dir.path(), "singbox".to_string()).unwrap();
        assert_eq!(versions, vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]);

        let mihomo = list_downloaded_versions_impl(dir.path(), "mihomo".to_string()).unwrap();
        assert_eq!(mihomo, vec!["1.19.29"]);

        // 无效 core_type 字符串报错。
        assert!(list_downloaded_versions_impl(dir.path(), "bogus".to_string()).is_err());
    }

    // ---------- 项 2.1：delete_core 命令层 ----------

    #[test]
    fn delete_core_deletes_downloaded_core_and_clears_version_dir() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "sing-box", "1.14.0");
        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");

        with_empty_path(|| delete_core_impl(dir.path(), &bin.to_string_lossy()).unwrap());

        assert!(!bin.exists(), "二进制应被删除");
        assert!(
            !dir.path().join("cores/sing-box/1.13.15").exists(),
            "版本目录应删除"
        );
        // 其他版本保留。
        assert!(dir.path().join("cores/sing-box/1.14.0/sing-box").exists());
    }

    #[test]
    fn delete_core_rejects_active_binary() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        let bin = dir.path().join("cores/sing-box/1.13.15/sing-box");
        // 配置 active 指向该核心。
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            bin.clone(),
        );
        cfg.save().unwrap();

        let err =
            with_empty_path(|| delete_core_impl(dir.path(), &bin.to_string_lossy()).unwrap_err());
        assert!(err.contains("正在使用的核心不可删除"), "{err}");
        assert!(bin.exists(), "active 核心不应被删除");
    }

    #[test]
    fn delete_core_rejects_system_source() {
        let dir = TestDir::new();
        // 在受控 PATH 下构造系统核心：`bin/sing-box`（可执行假脚本）。
        let system_bin = dir.path().join("bin/sing-box");
        std::fs::create_dir_all(system_bin.parent().unwrap()).unwrap();
        std::fs::write(&system_bin, b"#!/bin/sh\necho 'sing-box version 1.19.9'\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&system_bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let err = with_patched_path(&dir.path().join("bin"), || {
            delete_core_impl(dir.path(), &system_bin.to_string_lossy())
        })
        .unwrap_err();
        assert!(err.contains("系统核心不可删除"), "{err}");
        assert!(system_bin.exists(), "系统核心不应被删除");
    }

    #[test]
    fn delete_core_rejects_nonexistent_path() {
        let dir = TestDir::new();
        let missing = dir.path().join("cores/sing-box/9.9.9/sing-box");
        let err = with_empty_path(|| {
            delete_core_impl(dir.path(), &missing.to_string_lossy()).unwrap_err()
        });
        assert!(err.contains("不存在"), "{err}");
    }

    // ---------- MITM CA 证书命令 ----------

    #[test]
    fn get_mitm_ca_generates_and_reports_path() {
        let dir = TestDir::new();
        let view = get_mitm_ca_impl(dir.path()).unwrap();
        assert!(
            view.pem.contains("BEGIN CERTIFICATE"),
            "pem 应包含证书块: {}",
            view.pem
        );
        assert!(
            view.path.ends_with("ca.crt"),
            "path 应以 ca.crt 结尾: {}",
            view.path
        );
        assert!(
            std::path::Path::new(&view.path).is_file(),
            "CA 证书应已落盘: {}",
            view.path
        );

        // 幂等：已存在时不重新生成，再次调用返回同一证书。
        let again = get_mitm_ca_impl(dir.path()).unwrap();
        assert_eq!(view.pem, again.pem);
    }

    // ---------- 活动订阅（首页选中）+ 订阅关联覆写 ----------

    #[test]
    fn set_active_subscription_validates_and_persists_selection() {
        let dir = TestDir::new();
        // 预置 client.json。
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        // 不存在的订阅报错。
        let err =
            set_active_subscription_impl(dir.path(), Some(Uuid::new_v4().to_string())).unwrap_err();
        assert!(err.contains("不存在"), "{err}");

        // 已停用的订阅报错。
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let off = store
            .add("off", "https://example.com/sub", false, None)
            .unwrap();
        let err = set_active_subscription_impl(dir.path(), Some(off.id.to_string())).unwrap_err();
        assert!(err.contains("已停用"), "{err}");

        // 选中启用的订阅 → 写入 client.json。
        let on = store
            .add("on", "https://example.com/sub2", true, None)
            .unwrap();
        set_active_subscription_impl(dir.path(), Some(on.id.to_string())).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, Some(on.id));

        // None 清除选中。
        set_active_subscription_impl(dir.path(), None).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, None);
    }

    #[test]
    fn disabling_selected_subscription_clears_active_selection() {
        let dir = TestDir::new();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        let sub = store
            .add("sub", "https://example.com/sub", true, None)
            .unwrap();
        let mut cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.active_subscription_id = Some(sub.id);
        cfg.save().unwrap();

        // 停用选中订阅 → 自动清除选中。
        set_subscription_enabled_impl(dir.path(), sub.id, false).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, None);

        // 重新启用不影响（无选中可清）。
        set_subscription_enabled_impl(dir.path(), sub.id, true).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, None);

        // 停用未选中的订阅不影响选中状态。
        let other = store
            .add("other", "https://example.com/other", false, None)
            .unwrap();
        let mut cfg = ClientConfig::load(dir.path()).unwrap();
        cfg.active_subscription_id = Some(sub.id);
        cfg.save().unwrap();
        set_subscription_enabled_impl(dir.path(), other.id, false).unwrap();
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.active_subscription_id, Some(sub.id));
    }

    // ---------- 配置预览（指定订阅 / active 订阅路径） ----------

    /// 启动一个本地 HTTP mock 订阅服务器（无外部依赖）：raw TCP 监听，
    /// 每个连接先读取请求再返回固定 200 响应体。
    fn spawn_sub_server(body: &'static str) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else {
                    continue;
                };
                let mut buf = [0u8; 8192];
                // 先读请求再响应，避免对端请求未发完时关闭连接。
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

    /// sing-box JSON 格式的最小订阅（含 `outbounds`，嗅探为 `SingBoxJson`）。
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
        // enabled = false：预览指定订阅应忽略启用状态。
        let sub = store
            .add("spec", &format!("{base}/sub"), false, None)
            .unwrap();

        let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
            .await
            .expect("指定订阅预览应成功");
        let value: serde_json::Value = serde_json::from_str(&text).expect("sing-box 预览应为 JSON");
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

        let err = preview_core_config_impl(dir.path().to_path_buf(), Some(Uuid::new_v4()))
            .await
            .unwrap_err();
        assert!(err.contains("订阅不存在"), "{err}");
    }

    #[tokio::test]
    async fn preview_core_config_none_uses_active_subscription_selection() {
        let dir = TestDir::new();
        let store = SubscriptionStore::new(dir.path().to_path_buf());
        // 已停用的订阅：None 走 active 路径应报「已停用」，而非「订阅不存在」。
        let off = store
            .add("off", "https://example.com/sub", false, None)
            .unwrap();
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

        // 未选中订阅 + 无 Hub 配置 → 既有报错（active 路径的兜底分支）。
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

    // ---------- 规则模式（set_rule_mode 持久化） ----------

    #[test]
    fn set_rule_mode_rejects_invalid_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        for invalid in ["", "bogus", "Rule", "全局"] {
            let err = set_rule_mode_persist(dir.path(), invalid).unwrap_err();
            assert!(err.contains("无效的规则模式"), "{invalid:?}: {err}");
        }
    }

    #[test]
    fn set_rule_mode_persists_valid_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();

        // 默认 rule。
        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.rule_mode, "rule");

        for mode in ["global", "direct", "rule"] {
            set_rule_mode_persist(dir.path(), mode).unwrap();
            let saved = ClientConfig::load(dir.path()).unwrap();
            assert_eq!(saved.rule_mode, mode, "{mode} 应落盘 client.json");
        }
    }

    #[test]
    fn idle_status_view_reports_persisted_rule_mode() {
        let dir = TestDir::new();
        let cfg = ClientConfig::new(
            dir.path().to_path_buf(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        );
        cfg.save().unwrap();
        let view = idle_status_view(dir.path());
        assert_eq!(view.rule_mode, "rule");
        assert_eq!(view.rule_count, 0);
        assert!(!view.core_running);
        assert!(view.clash_api_url.is_none());

        set_rule_mode_persist(dir.path(), "direct").unwrap();
        let view = idle_status_view(dir.path());
        assert_eq!(view.rule_mode, "direct");
    }

    // ---------- GPU 加速检测（gpu_acceleration） ----------

    #[test]
    fn gpu_acceleration_native_linux_has_gpu_unless_forced_software() {
        // 原生 Linux（osrelease 无 WSL 标识）+ 未强制软渲染 → 有 GPU（保守）。
        assert!(gpu_acceleration_impl(
            Some("Linux version 6.8.0-generic"),
            false,
            None,
        ));
        // dxg 存在与否不影响原生 Linux 判定。
        assert!(gpu_acceleration_impl(
            Some("Linux version 6.8.0-generic"),
            true,
            None,
        ));
        assert!(gpu_acceleration_impl(
            Some("Linux version 6.8.0-generic"),
            false,
            Some("0"),
        ));
        // 原生 Linux + LIBGL_ALWAYS_SOFTWARE=1 → 无 GPU。
        assert!(!gpu_acceleration_impl(
            Some("Linux version 6.8.0-generic"),
            false,
            Some("1"),
        ));
        // osrelease 读取失败视为非 WSL → 按原生 Linux 保守判定有 GPU。
        assert!(gpu_acceleration_impl(None, false, None));
    }

    #[test]
    fn gpu_acceleration_wsl_requires_dxg_and_no_forced_software() {
        // WSL + dxg 存在 + 未强制软渲染 → 有 GPU。
        assert!(gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-microsoft-standard-WSL2"),
            true,
            None,
        ));
        assert!(gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-microsoft-standard-WSL2"),
            true,
            Some("0"),
        ));
        // WSL 无 dxg（无 WSLg GPU 直通）→ 无 GPU（软渲染兜底）。
        assert!(!gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-microsoft-standard-WSL2"),
            false,
            None,
        ));
        // WSL + dxg 但强制软渲染 → 无 GPU。
        assert!(!gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-microsoft-standard-WSL2"),
            true,
            Some("1"),
        ));
    }

    #[test]
    fn gpu_acceleration_osrelease_matches_wsl_ignore_case() {
        // osrelease 忽略大小写匹配 microsoft / wsl。
        assert!(gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-MICROSOFT-standard-WSL2"),
            true,
            None,
        ));
        assert!(gpu_acceleration_impl(Some("microsoft"), true, None));
        assert!(gpu_acceleration_impl(Some("  wsl2 kernel  "), true, None));
    }
}
