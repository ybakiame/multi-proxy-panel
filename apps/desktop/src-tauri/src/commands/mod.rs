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

/// Unified error prefix for commands unavailable on Android.
#[cfg(target_os = "android")]
const UNSUPPORTED_PLATFORM_PREFIX: &str = "unsupported_platform";

/// Returns a unified error when called on Android; desktop path is cfg'd out.
#[cfg(target_os = "android")]
fn require_desktop<T>(feature: &str) -> Result<T, String> {
    Err(format!("{UNSUPPORTED_PLATFORM_PREFIX}: {feature} is not supported on Android"))
}

// Shared command-layer helpers (双端通用) live in `pp-client-tauri`; re-export here
// so existing `crate::commands::*` references in shell command modules stay unchanged.
pub use pp_client_tauri::commands::{
    TauriNotifier, parse_profile_id, remote_kind_str, script_dialect_str, sub_format_str,
};
