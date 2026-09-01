//! Tauri command modules organized by functional domain.
//!
//! Each submodule owns a distinct feature area; `mod.rs` only declares modules
//! and aggregates the `generate_handler!` registration list.

mod config;
mod connections;
mod core_mgmt;
mod local_override;
mod mitm;
mod platform;
mod preview;
mod profile;
mod proxies;
mod proxy;
mod remote;
mod subscription;
mod task;

pub use config::*;
pub use connections::*;
pub use core_mgmt::*;
pub use local_override::*;
pub use mitm::*;
pub use platform::*;
pub use preview::*;
pub use profile::*;
pub use proxies::*;
pub use proxy::*;
pub use remote::*;
pub use subscription::*;
pub use task::*;

use pp_client::SubFormat;
use pp_common::CoreType;
use pp_script::ScriptDialect;
use uuid::Uuid;

/// Unified error prefix for commands unavailable on Android.
const UNSUPPORTED_PLATFORM_PREFIX: &str = "unsupported_platform";

/// Returns a unified error when called on Android; desktop path is unreachable.
#[cfg(target_os = "android")]
fn require_desktop<T>(feature: &str) -> Result<T, String> {
    Err(format!("{UNSUPPORTED_PLATFORM_PREFIX}: {feature} is not supported on Android"))
}

/// Desktop path: unreachable (guarded by `cfg` before calling).
#[cfg(not(target_os = "android"))]
fn require_desktop<T>(_: &str) -> Result<T, String> {
    unreachable!("require_desktop should only be called after cfg guard")
}

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
fn parse_profile_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("invalid profile ID: {e}"))
}

// The following pure conversion functions are re-exported from pp_client::validation
// for backward compatibility with existing command modules.

/// Serializes `CoreType` to frontend lowercase convention (`singbox` / `mihomo`).
fn core_type_str(core_type: CoreType) -> String {
    pp_client::core_type_str(core_type)
}

/// Parses frontend lowercase core type string (`singbox` / `mihomo`).
fn core_type_from_str(s: &str) -> Result<CoreType, String> {
    pp_client::core_type_from_str(s)
}

/// String representation of `RemoteKind` (matches `RemoteResourceView.kind` serde).
fn remote_kind_str(kind: pp_client::RemoteKind) -> &'static str {
    pp_client::remote_kind_str(kind)
}

/// String representation of `ScriptDialect` (matches `RemoteResourceView.dialect` serde).
///
/// QX is merged into the Loon ecosystem; detected QuantumultX is mapped to `Loon`.
fn script_dialect_str(dialect: ScriptDialect) -> &'static str {
    pp_client::script_dialect_str(dialect)
}

/// String representation of `SubFormat`.
fn sub_format_str(format: SubFormat) -> &'static str {
    pp_client::sub_format_str(format)
}
