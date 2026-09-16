//! Shared command layer for the `pp-client-tauri` crate.
//!
//! Desktop / Mobile 壳以路径注册双端通用命令（见 ADR-0003 §3.2）：[`baseline_view`] /
//! [`config`] / [`config_slices`] / [`profile`] / [`subscription`] / [`preview`] 与 [`proxy`] /
//! [`proxies`] / [`connections`] / [`task`] / [`local_override`] 为双端通用命令
//! 模块（其中 [`config_slices`] 为 ADR-0005 新增），经 `pub use` glob 聚合到本层，
//! 壳以 `pp_client_tauri::commands::<name>` 注册。其余为命令共享辅助：桌面通知器
//! （[`TauriNotifier`]）与纯转换/解析函数。

mod baseline_view;
mod config;
mod config_slices;
mod connections;
mod diagnose;
mod dns_probe;
mod local_override;
mod preview;
mod profile;
mod proxies;
mod proxy;
mod subscription;
mod subscription_nodes;
mod task;

pub use baseline_view::*;
pub use config::*;
pub use config_slices::*;
pub use connections::*;
pub use diagnose::*;
pub use dns_probe::*;
pub use local_override::*;
pub use preview::*;
pub use profile::*;
pub use proxies::*;
pub use proxy::*;
pub use subscription::*;
pub use subscription_nodes::*;
pub use task::*;

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
