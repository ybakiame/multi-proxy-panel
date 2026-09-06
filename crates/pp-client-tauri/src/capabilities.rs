//! Platform capability matrix commands (shared across Desktop/Mobile shells).

use serde::Serialize;

/// Platform capability matrix view.
#[derive(Debug, Clone, Serialize)]
pub struct CapabilitiesView {
    pub os: String,
    pub is_android: bool,
    pub capabilities: PlatformCapabilities,
}

/// Per-platform feature capability matrix.
#[derive(Debug, Clone, Serialize)]
pub struct PlatformCapabilities {
    pub mitm: bool,
    pub system_proxy: bool,
    pub core_management: bool,
    pub tun_toggle: bool,
    pub scripts_remote: bool,
    pub cron_tasks: bool,
}

impl CapabilitiesView {
    fn current() -> Self {
        let os = std::env::consts::OS.to_string();
        let is_android = cfg!(target_os = "android");
        Self {
            os: os.clone(),
            is_android,
            capabilities: PlatformCapabilities {
                mitm: !is_android,
                system_proxy: !is_android,
                core_management: !is_android,
                tun_toggle: !is_android,
                scripts_remote: !is_android,
                cron_tasks: true,
            },
        }
    }
}

/// Query platform capability matrix.
#[tauri::command]
pub fn get_capabilities() -> CapabilitiesView {
    CapabilitiesView::current()
}

/// Legacy `platform_info` (compatibility shim).
#[tauri::command]
pub fn platform_info() -> CapabilitiesView {
    CapabilitiesView::current()
}
