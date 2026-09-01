//! Platform, TUN, and rendering compatibility commands.

use serde::Serialize;
use tauri::State;

use crate::state::AppState;

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

/// TUN authorization status based on current `core_binary`.
#[tauri::command]
pub async fn tun_auth_status(state: State<'_, AppState>) -> Result<String, String> {
    let cfg = ClientConfig::load(&state.data_dir).map_err(|e| format!("读取配置失败: {e}"))?;
    Ok(pp_client::tun_auth_status(&cfg.core_binary).as_frontend_str())
}

/// Execute TUN authorization.
#[tauri::command]
pub async fn authorize_tun(state: State<'_, AppState>) -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        let _ = state;
        return require_desktop("TUN authorization");
    }
    #[cfg(not(target_os = "android"))]
    {
        let cfg = ClientConfig::load(&state.data_dir).map_err(|e| format!("读取配置失败: {e}"))?;
        pp_client::authorize_tun(&cfg.core_binary).map_err(|e| e.to_string())?;
        Ok(pp_client::tun_auth_status(&cfg.core_binary).as_frontend_str())
    }
}

/// Request VPN permission (Android only).
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

/// Read last VPN start error (Android only).
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

/// Notify the Kotlin VpnPlugin that notification preferences have changed (Android only).
#[cfg(target_os = "android")]
#[tauri::command]
pub async fn notify_prefs_changed(show_traffic: bool, show_selection: bool) -> Result<(), String> {
    let handle = crate::core_bridge::vpn_plugin_handle()
        .ok_or_else(|| "VPN plugin not initialized, please restart the app".to_string())?;
    let payload = serde_json::json!({
        "showTraffic": show_traffic,
        "showSelection": show_selection,
    });
    handle
        .run_mobile_plugin_async::<serde_json::Value>("updateNotifyPrefs", payload)
        .await
        .map(|_| ())
        .map_err(|e| format!("Failed to update notification preferences: {e}"))
}

/// GPU acceleration detection.
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

/// Pure logic for GPU acceleration (parameterized for testability).
pub(crate) fn gpu_acceleration_impl(
    os_release: Option<&str>,
    has_dxg: bool,
    libgl_always_software: Option<&str>,
) -> bool {
    if !cfg!(target_os = "linux") {
        return true;
    }
    if os_release.is_some_and(crate::is_wsl_osrelease) {
        return has_dxg && libgl_always_software.is_none_or(|v| v == "0");
    }
    libgl_always_software.is_none_or(|v| v != "1")
}

/// Read toast rendering mode override from `PP_TOAST_MODE`.
#[tauri::command]
pub fn toast_mode_override() -> Option<String> {
    std::env::var("PP_TOAST_MODE")
        .ok()
        .filter(|v| !v.trim().is_empty())
}

use pp_client::ClientConfig;
#[cfg(target_os = "android")]
use super::require_desktop;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gpu_acceleration_native_linux_has_gpu_unless_forced_software() {
        assert!(gpu_acceleration_impl(
            Some("Linux version 6.8.0-generic"),
            false,
            None,
        ));
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
        assert!(!gpu_acceleration_impl(
            Some("Linux version 6.8.0-generic"),
            false,
            Some("1"),
        ));
        assert!(gpu_acceleration_impl(None, false, None));
    }

    #[test]
    fn gpu_acceleration_wsl_requires_dxg_and_no_forced_software() {
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
        assert!(!gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-microsoft-standard-WSL2"),
            false,
            None,
        ));
        assert!(!gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-microsoft-standard-WSL2"),
            true,
            Some("1"),
        ));
    }

    #[test]
    fn gpu_acceleration_osrelease_matches_wsl_ignore_case() {
        assert!(gpu_acceleration_impl(
            Some("Linux version 5.15.133.1-MICROSOFT-standard-WSL2"),
            true,
            None,
        ));
        assert!(gpu_acceleration_impl(Some("microsoft"), true, None));
        assert!(gpu_acceleration_impl(Some("  wsl2 kernel  "), true, None));
    }
}
