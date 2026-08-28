//! Compatibility and utility free functions for [`ClientState`].

use pp_common::{CoreType, PanelError, PanelResult};

#[cfg(any(test, target_os = "android"))]
use crate::config::ClientConfig;
use crate::subscription;

/// Android startup forced override: desktop-exclusive features have no corresponding implementation on Android
/// (or would break VpnService takeover).
///
/// Android now supports dual cores (panelcore.aar bundles sing-box libbox + mihomo),
/// `core_type` respects user config, no longer forced to sing-box.
///
/// Reasons for each forced item:
/// - `mitm_enabled = false`: MITM depends on P3 features, libbox does not support, disable to avoid chain injection;
/// - `system_proxy_enabled = false`: Android system proxy is stub, calling it will error;
/// - `tun_enabled = false`: only affects this struct, drives desktop TUN pre-start privilege check
///   (gated by `#[cfg(not(target_os = "android"))]` in [`ClientState::start`]) and settings page
///   UI semantics; Android traffic is taken over by VpnService (i.e. TUN), config composition differs by core type:
///   sing-box must inject tun inbound to trigger libbox `openTun()` callback to establish VPN interface,
///   mihomo on Android is TUN-driven by wrapper with fd (wrapper already forces `Tun.Enable=false`), no tun section at config level,
///   so [`ClientState::start`] constructs [`core_config::PanelFeatures`] on Android via [`panel_features_tun_enabled`] by core type
///   (the false here does not participate in config composition).
///
/// Only called by [`ClientState::start`] on Android builds; compiled on desktop builds for unit test verification.
#[cfg(any(test, target_os = "android"))]
pub(crate) fn apply_android_overrides(config: &mut ClientConfig) {
    config.mitm_enabled = false;
    config.system_proxy_enabled = false;
    config.tun_enabled = false;
    tracing::info!(
        "Android forced override: core type keeps user config (sing-box / mihomo dual core), disables MITM / system proxy / TUN (desktop semantics only, tun injected by core type in config composition)"
    );
}

/// Recursively redact credential fields in config (for Android troubleshooting disk write): when object key is
/// "uuid" / "password" / "server" and value is string, replace with "***",
/// other structures (including detour / dns levels) are preserved as-is.
#[cfg(any(test, target_os = "android"))]
pub(crate) fn redact_config_credentials(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if matches!(key.as_str(), "uuid" | "password" | "server") && val.is_string() {
                    *val = serde_json::Value::String("***".to_string());
                } else {
                    redact_config_credentials(val);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items.iter_mut() {
                redact_config_credentials(item);
            }
        }
        _ => {}
    }
}

/// PanelFeatures TUN toggle: Android traffic is taken over by VpnService, distinguished by core type —
/// sing-box needs config-level tun inbound to trigger libbox callback `openTun()` to establish VPN interface
/// (always true); mihomo on Android is TUN-driven by wrapper with fd (wrapper Setup already forces
/// `Tun.Enable=false`), no tun section injected at config level (always false). Desktop passes through user settings as-is.
pub(crate) fn panel_features_tun_enabled(
    is_android: bool,
    core_type: CoreType,
    tun_enabled: bool,
) -> bool {
    if is_android {
        matches!(core_type, CoreType::SingBox)
    } else {
        tun_enabled
    }
}

/// Pure logic for subscription format ↔ core type compatibility check,
/// parameterized by `is_android` so it can be unit-tested on desktop builds.
///
/// - `SubFormat::SingBoxJson` → only sing-box core;
/// - `SubFormat::ClashYaml` → only mihomo core (on Android, auto-downgrade
///   from sing-box to mihomo instead of hard error);
/// - `SubFormat::ShareLinks` → both cores.
pub(crate) fn check_subscription_core_compat_pure(
    format: subscription::SubFormat,
    core_type: CoreType,
    subscription_id: Option<uuid::Uuid>,
    is_android: bool,
) -> PanelResult<CoreType> {
    let compatible = match format {
        subscription::SubFormat::ShareLinks => true,
        subscription::SubFormat::SingBoxJson => core_type == CoreType::SingBox,
        subscription::SubFormat::ClashYaml => core_type == CoreType::Mihomo,
    };
    if compatible {
        return Ok(core_type);
    }

    // Android: auto-downgrade clash → mihomo instead of hard error
    if is_android && format == subscription::SubFormat::ClashYaml && core_type == CoreType::SingBox
    {
        if let Some(id) = subscription_id {
            tracing::warn!(
                subscription_id = %id,
                "Android auto-downgrade: clash 订阅与 sing-box 不兼容，切换核心 singbox→mihomo"
            );
        } else {
            tracing::warn!(
                "Android auto-downgrade: clash 订阅与 sing-box 不兼容，切换核心 singbox→mihomo"
            );
        }
        return Ok(CoreType::Mihomo);
    }

    let (format_name, supported_core) = match format {
        subscription::SubFormat::ClashYaml => ("clash", "mihomo"),
        subscription::SubFormat::SingBoxJson => ("sing-box", "sing-box"),
        subscription::SubFormat::ShareLinks => {
            unreachable!("ShareLinks supports both cores, should not reach mismatch branch")
        }
    };
    Err(PanelError::Client(format!(
        "订阅格式为 {format_name}，仅支持 {supported_core} 核心，当前核心类型为 {core_type}，请在设置中切换核心类型"
    )))
}

/// Subscription format ↔ core type compatibility check.
///
/// Delegates to [`check_subscription_core_compat_pure`] with the actual
/// platform flag (`target_os = "android"`).
pub(crate) fn check_subscription_core_compat(
    format: subscription::SubFormat,
    core_type: CoreType,
    subscription_id: Option<uuid::Uuid>,
) -> PanelResult<CoreType> {
    check_subscription_core_compat_pure(
        format,
        core_type,
        subscription_id,
        cfg!(target_os = "android"),
    )
}

/// User-visible display name of core type (`sing-box` / `mihomo`), used in override matching error messages.
pub fn core_type_display_name(core_type: CoreType) -> &'static str {
    match core_type {
        CoreType::SingBox => "sing-box",
        CoreType::Mihomo => "mihomo",
    }
}

/// Calculate number of rules in composed config: sing-box takes `route.rules` array length, mihomo takes top-level
/// `rules` array length (missing array treated as 0).
pub(crate) fn config_json_rule_count(config_json: &serde_json::Value, core_type: CoreType) -> u64 {
    match core_type {
        CoreType::SingBox => config_json
            .get("route")
            .and_then(|r| r.get("rules"))
            .and_then(|rules| rules.as_array())
            .map_or(0, |rules| rules.len() as u64),
        CoreType::Mihomo => config_json
            .get("rules")
            .and_then(|rules| rules.as_array())
            .map_or(0, |rules| rules.len() as u64),
    }
}
