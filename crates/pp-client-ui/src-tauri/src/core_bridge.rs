//! Android 核心引擎桥（真实实现：Rust ↔ Kotlin VpnPlugin/libbox 通道）。
//!
//! Android 上核心由 Kotlin 侧 libbox（VpnService）驱动，Rust 侧无法 spawn
//! 二进制。本模块在 `run()` 的 `vpn` 插件 setup 中经
//! `tauri::plugin::PluginApi::register_android_plugin` 注册
//! `com.proxypanel.client` 的 `VpnPlugin`，拿到 `PluginHandle` 后安装真实桥：
//! `start` / `stop` / `is_running` 通过 `run_mobile_plugin_async` 转发给
//! Kotlin 插件，核心生命周期由 libbox 执行。
//!
//! 错误透传：Kotlin 侧 `invoke.reject(..., "vpn_not_authorized")` 的拒绝
//! 在 [`plugin_error`] 中被规范为可识别前缀 `vpn_not_authorized: ...` 上抛，
//! 前端据此展示「需要 VPN 授权」引导。

use std::sync::Arc;

use pp_client::core_engine::{install_core_engine_bridge, BoxFuture, CoreEngineBridge};
use pp_common::{PanelError, PanelResult};
use serde::Deserialize;
use serde_json::Value;
use tauri::plugin::mobile::PluginInvokeError;
use tauri::plugin::PluginHandle;

/// VpnPlugin `isRunning` 命令的响应体（`pub` 供 `commands::vpn_last_error` 读取）。
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
    fn start<'a>(&'a self, config_json: &'a Value) -> BoxFuture<'a, PanelResult<()>> {
        Box::pin(async move {
            // 与 Kotlin `StartArgs`（`config: String`）对齐：配置以 JSON 文本传递。
            let payload = serde_json::json!({ "config": config_json.to_string() });
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
