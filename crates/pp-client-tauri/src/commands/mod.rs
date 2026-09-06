//! Shared command helpers for the `pp-client-tauri` command layer.
//!
//! Desktop / Mobile 壳以路径注册双端通用命令（见 ADR-0003 §3.2）；本模块承载命令
//! 实现的共享辅助：桌面通知器（[`TauriNotifier`]）与纯转换/解析函数。壳内命令模块
//! 通过 `pub use pp_client_tauri::commands::{...}` 维持既有 `commands::*` 引用。

use pp_client::SubFormat;
use pp_script::ScriptDialect;
use uuid::Uuid;

/// OS desktop notifier backed by `tauri-plugin-notification`.
///
/// Falls back to `tracing::warn` on failure without blocking script execution.
pub struct TauriNotifier {
    app: tauri::AppHandle,
}

impl TauriNotifier {
    /// Creates a notifier from the app handle.
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
            tracing::warn!(error = %e, "failed to send desktop notification");
        }
    }
}

/// Parses a profile ID string into `Uuid`.
pub fn parse_profile_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("invalid profile ID: {e}"))
}

// The following pure conversion functions forward to pp_client helpers, kept here
// so shell command modules share a single import surface.

/// String representation of `RemoteKind` (matches `RemoteResourceView.kind` serde).
pub fn remote_kind_str(kind: pp_client::RemoteKind) -> &'static str {
    pp_client::remote_kind_str(kind)
}

/// String representation of `ScriptDialect` (matches `RemoteResourceView.dialect` serde).
///
/// QX is merged into the Loon ecosystem; detected QuantumultX is mapped to `Loon`.
pub fn script_dialect_str(dialect: ScriptDialect) -> &'static str {
    pp_client::script_dialect_str(dialect)
}

/// String representation of `SubFormat`.
pub fn sub_format_str(format: SubFormat) -> &'static str {
    pp_client::sub_format_str(format)
}
