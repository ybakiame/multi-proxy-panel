//! Desktop-only Tauri command modules.
//!
//! 双端通用命令已迁移至 `pp-client-tauri` 共享层（见 ADR-0003 §3.2/§3.3），本壳仅
//! 保留桌面专属模块（mitm / core_mgmt / remote / platform）与 `require_desktop`
//! Android 守卫。双端通用命令经 `generate_handler!` 以 `pp_client_tauri::commands::
//! <name>` 全路径注册（Tauri 2 支持跨 crate 注册）。

mod core_mgmt;
mod mitm;
mod platform;
mod remote;

pub use core_mgmt::*;
pub use mitm::*;
pub use platform::*;
pub use remote::*;

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
