//! Android 核心引擎桥与 Android 专属命令（真实实现：Rust ↔ Kotlin
//! VpnPlugin/libbox 通道）。
//!
//! Android 上核心由 Kotlin 侧 VpnPlugin 驱动（sing-box libbox，经 panelcore.aar
//! 绑定），Rust 侧无法 spawn 二进制。本模块在壳 `run()` 的 `vpn` 插件 setup 中经
//! `tauri::plugin::PluginApi::register_android_plugin` 注册
//! `com.proxypanel.client` 的 `VpnPlugin`，拿到 `PluginHandle` 后安装真实桥：
//! `start` / `stop` / `is_running` 通过 `run_mobile_plugin_async` 转发给
//! Kotlin 插件，核心生命周期由 libbox 执行。
//!
//! 配置下发：配置以 JSON 文本经 `config` 字段传给 Kotlin；客户端仅支持
//! sing-box，`StartArgs` 不再携带核心分派参数（仅 `config` / `showSelection` /
//! `subscriptionName`）。
//!
//! 错误透传：Kotlin 侧 `invoke.reject(..., "vpn_not_authorized")` 的拒绝
//! 在 [`plugin_error`] 中被规范为可识别前缀 `vpn_not_authorized: ...` 上抛，
//! 前端据此展示「需要 VPN 授权」引导。
//!
//! 本模块还承载 Android 专属三命令（[`request_vpn_permission`] /
//! [`vpn_last_error`] / [`notify_prefs_changed`]，ADR-0003 M3.5 自 desktop 壳
//! `commands/platform.rs` 上移）。模块整体以 `#[cfg(target_os = "android")]`
//! 编译裁剪，桌面构建不含任何本模块代码；壳层注册 `vpn` 插件与三命令时仍以
//! `#[cfg(target_os = "android")]` 显式包裹（过渡态，M4 清理）。

use std::sync::Arc;

use pp_client::core_engine::{BoxFuture, CoreEngineBridge, install_core_engine_bridge};
use pp_common::{CoreType, PanelError, PanelResult};
use serde::Deserialize;
use serde_json::Value;
use tauri::plugin::PluginHandle;
use tauri::plugin::mobile::PluginInvokeError;

/// VpnPlugin `isRunning` 命令的响应体（`pub` 供 [`vpn_last_error`] 读取）。
#[derive(Deserialize)]
pub struct IsRunningResponse {
    pub running: bool,
    /// 最近一次启动失败原因（无失败 / 未设置时缺失或为 null，回退 `None`）。
    #[serde(default)]
    pub last_error: Option<String>,
}

/// 已注册 `vpn` 插件的句柄（供 [`request_vpn_permission`] 命令调用 `prepare`）。
static VPN_PLUGIN_HANDLE: std::sync::OnceLock<PluginHandle<tauri::Wry>> =
    std::sync::OnceLock::new();

/// 应用数据目录（移动壳 setup 时经 [`set_data_dir`] 注入一次），供 VPN 通知
/// 偏好与生效订阅名解析（`start` 收到的 config_json 是 sing-box 配置，不含
/// 客户端偏好字段，必须回源 `client.json`）。
static DATA_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// 注入应用数据目录（移动壳 setup 调用一次；重复注入静默忽略）。
pub fn set_data_dir(dir: std::path::PathBuf) {
    let _ = DATA_DIR.set(dir);
}

/// VPN 通知展示上下文：是否展示节点选择 + 生效订阅名称。
struct NotifyContext {
    show_selection: bool,
    subscription_name: Option<String>,
}

/// 从 `client.json` 与 `subscriptions.json` 解析通知展示上下文（文件缺失/损坏时
/// 回退默认：显示节点选择、无订阅名）。
fn notify_context(data_dir: &std::path::Path) -> NotifyContext {
    let show_selection = pp_client::ClientConfig::load(data_dir)
        .map(|cfg| cfg.vpn_notify_show_selection)
        .unwrap_or(true);
    let subscription_name = pp_client::ClientConfig::load(data_dir)
        .ok()
        .and_then(|cfg| cfg.active_subscription_id)
        .and_then(|id| {
            pp_client::SubscriptionStore::new(data_dir.to_path_buf())
                .load()
                .ok()?
                .into_iter()
                .find(|sub| sub.id == id)
                .map(|sub| sub.name)
        });
    NotifyContext {
        show_selection,
        subscription_name,
    }
}

/// 真实桥：持有已注册的 `vpn` 插件句柄，把核心生命周期转发给 Kotlin libbox。
pub struct AndroidCoreBridge {
    handle: PluginHandle<tauri::Wry>,
}

impl AndroidCoreBridge {
    /// 构造真实桥（`vpn` 插件已注册完成）。
    pub fn new(handle: PluginHandle<tauri::Wry>) -> Self {
        Self { handle }
    }
}

/// 把移动插件调用错误映射为 [`PanelError`]。
///
/// Kotlin 侧 `invoke.reject(msg, code)` 的拒绝会落到 `PluginInvokeError::InvokeRejected`，
/// 其中 `code = "vpn_not_authorized"` 表示需要系统 VPN 授权，以可识别前缀
/// `vpn_not_authorized: <message>` 上抛给前端；其余拒绝保留 `[code] message` 形态
/// （无 code 时仅 message）。`pub(crate)` 供 [`crate::logs::export_logs`] 复用。
pub(crate) fn plugin_error(err: PluginInvokeError) -> PanelError {
    match err {
        PluginInvokeError::InvokeRejected(resp) => {
            let message = resp
                .message
                .as_deref()
                .unwrap_or("vpn plugin command rejected");
            match resp.code.as_deref() {
                Some("vpn_not_authorized") => {
                    PanelError::Client(format!("vpn_not_authorized: {message}"))
                }
                Some(code) => PanelError::Client(format!("[{code}] {message}")),
                None => PanelError::Client(message.to_string()),
            }
        }
        other => PanelError::Client(other.to_string()),
    }
}

impl CoreEngineBridge for AndroidCoreBridge {
    fn start<'a>(
        &'a self,
        _core_type: CoreType,
        config_json: &'a Value,
    ) -> BoxFuture<'a, PanelResult<()>> {
        Box::pin(async move {
            // 通知偏好与生效订阅名回源 client.json / subscriptions.json（config_json
            // 是 sing-box 配置，不含客户端字段）；data_dir 未注入（host 编译检查等
            // 异常路径）时回退默认：显示节点选择、无订阅名。
            let ctx = DATA_DIR
                .get()
                .map(|dir| notify_context(dir))
                .unwrap_or(NotifyContext {
                    show_selection: true,
                    subscription_name: None,
                });
            // 与 Kotlin `StartArgs` 对齐（`config: String` + 通知偏好 + 订阅名）：
            // 客户端仅支持 sing-box，核心类型不再下发，配置以 JSON 文本传递。
            let payload = serde_json::json!({
                "config": config_json.to_string(),
                "showSelection": ctx.show_selection,
                "subscriptionName": ctx.subscription_name,
            });
            self.handle
                .run_mobile_plugin_async::<Value>("start", payload)
                .await
                .map(|_| ())
                .map_err(plugin_error)
        })
    }

    fn stop<'a>(&'a self) -> BoxFuture<'a, PanelResult<()>> {
        Box::pin(async move {
            self.handle
                .run_mobile_plugin_async::<Value>("stop", ())
                .await
                .map(|_| ())
                .map_err(plugin_error)
        })
    }

    fn is_running<'a>(&'a self) -> BoxFuture<'a, bool> {
        Box::pin(async move {
            self.handle
                .run_mobile_plugin_async::<IsRunningResponse>("isRunning", ())
                .await
                .map(|resp| resp.running)
                .unwrap_or(false)
        })
    }
}

/// 安装真实桥并记录插件句柄（在 `vpn` 插件 setup 中调用一次）。
///
/// 句柄同时存入静态注册表（[`vpn_plugin_handle`]），供
/// `request_vpn_permission` 命令发起系统 VPN 授权跳转。
pub fn install_android_core_bridge(handle: PluginHandle<tauri::Wry>) {
    match install_core_engine_bridge(Arc::new(AndroidCoreBridge::new(handle.clone()))) {
        Ok(()) => tracing::info!("已安装 Android 核心引擎桥（VpnPlugin 真实通道）"),
        Err(e) => tracing::warn!("核心引擎桥安装失败：{e}"),
    }
    // 插件句柄只安装一次（OnceLock），重复安装静默忽略。
    let _ = VPN_PLUGIN_HANDLE.set(handle);
}

/// 已注册 `vpn` 插件句柄（未注册 / 已安装完成前返回 `None`）。
pub fn vpn_plugin_handle() -> Option<PluginHandle<tauri::Wry>> {
    VPN_PLUGIN_HANDLE.get().cloned()
}

/// 构建 `vpn` 移动插件：注册 Kotlin `VpnPlugin` 并安装真实核心引擎桥。
///
/// 桌面端不参与编译（插件仅 Android 注册），setup 在 Android 上经
/// `register_android_plugin("com.proxypanel.client", "VpnPlugin")` 实例化
/// Kotlin 插件（构造签名 `(Activity)`，符合 tauri 2.11 反射要求）。
pub fn vpn_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("vpn")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                let handle = _api.register_android_plugin("com.proxypanel.client", "VpnPlugin")?;
                install_android_core_bridge(handle);
            }
            Ok(())
        })
        .build()
}

/// Request VPN permission (Android only).
///
/// 迁移自 desktop 壳 `commands/platform.rs`（ADR-0003 M3.5），逻辑原样：向已注册的
/// Kotlin VpnPlugin 发起 `prepare` 触发系统 VPN 授权。
#[tauri::command]
pub async fn request_vpn_permission() -> Result<(), String> {
    let handle =
        vpn_plugin_handle().ok_or_else(|| "VPN 插件未初始化，请重启应用后重试".to_string())?;
    handle
        .run_mobile_plugin_async::<serde_json::Value>("prepare", ())
        .await
        .map(|_| ())
        .map_err(|e| format!("VPN 授权失败: {e}"))
}

/// Read last VPN start error (Android only).
///
/// 迁移自 desktop 壳 `commands/platform.rs`（ADR-0003 M3.5），逻辑原样：经 `isRunning`
/// 响应读取最近一次启动失败原因。
#[tauri::command]
pub async fn vpn_last_error() -> Option<String> {
    let handle = vpn_plugin_handle()?;
    let resp = handle
        .run_mobile_plugin_async::<IsRunningResponse>("isRunning", ())
        .await
        .ok()?;
    resp.last_error
}

/// Read the sing-box version compiled into `panelcore.aar` (Android only).
///
/// 经 `vpn` 插件的 `coreVersion` 命令调用 Kotlin `Libbox.version()`，返回构建期
/// ldflags 注入的 sing-box 版本（如 `1.14.0`）。关于页据此动态展示核心版本，
/// 无需前端硬编码；版本由 build-panel-core.sh 的 `-X ...constant.Version` 决定。
#[tauri::command]
pub async fn core_version() -> Result<String, String> {
    let handle =
        vpn_plugin_handle().ok_or_else(|| "VPN 插件未初始化，请重启应用后重试".to_string())?;
    handle
        .run_mobile_plugin_async::<String>("coreVersion", ())
        .await
        .map_err(|e| format!("读取 sing-box 版本失败: {e}"))
}

/// Notify the Kotlin VpnPlugin that notification preferences have changed (Android only).
///
/// 迁移自 desktop 壳 `commands/platform.rs`（ADR-0003 M3.5）：把通知偏好
/// （`showSelection`）与当前生效订阅名经 `updateNotifyPrefs` 下发给 Kotlin
/// VpnPlugin；实时流量展示已移除（实用性低），订阅名随核心启动时解析，此处
/// 一并热更新保证运行中切换订阅后通知内容准确。
#[tauri::command]
pub async fn notify_prefs_changed(
    state: tauri::State<'_, crate::state::AppState>,
    show_selection: bool,
) -> Result<(), String> {
    let handle = vpn_plugin_handle()
        .ok_or_else(|| "VPN plugin not initialized, please restart the app".to_string())?;
    let ctx = notify_context(&state.data_dir);
    let payload = serde_json::json!({
        "showSelection": show_selection,
        "subscriptionName": ctx.subscription_name,
    });
    handle
        .run_mobile_plugin_async::<serde_json::Value>("updateNotifyPrefs", payload)
        .await
        .map(|_| ())
        .map_err(|e| format!("Failed to update notification preferences: {e}"))
}
