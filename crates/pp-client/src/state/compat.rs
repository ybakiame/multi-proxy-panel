//! Compatibility and utility free functions for [`ClientState`].

#[cfg(any(test, target_os = "android"))]
use crate::config::ClientConfig;

/// Android startup forced override: desktop-exclusive features have no corresponding implementation on Android
/// (or would break VpnService takeover).
///
/// Reasons for each forced item:
/// - `mitm_enabled = false`: MITM depends on P3 features, libbox does not support, disable to avoid chain injection;
/// - `system_proxy_enabled = false`: Android system proxy is stub, calling it will error;
/// - `tun_enabled = false`: only affects this struct, drives desktop TUN pre-start privilege check
///   (gated by `#[cfg(not(target_os = "android"))]` in [`ClientState::start`]) and settings page
///   UI semantics; Android traffic is taken over by VpnService (i.e. TUN), sing-box must inject
///   tun inbound to trigger libbox `openTun()` callback to establish VPN interface, so
///   [`ClientState::start`] constructs [`core_config::PanelFeatures`] on Android via
///   [`panel_features_tun_enabled`] (the false here does not participate in config composition).
/// - `clash_api_enabled = true`: Clash API is a mandatory slice (ADR-0005 P1), the data source for
///   the dashboard / node page; always enabled on Android regardless of the persisted value.
///
/// Only called by [`ClientState::start`] on Android builds; compiled on desktop builds for unit test verification.
///
/// [`core_config::PanelFeatures`]: crate::core_config::PanelFeatures
#[cfg(any(test, target_os = "android"))]
pub(crate) fn apply_android_overrides(config: &mut ClientConfig) {
    config.mitm_enabled = false;
    config.system_proxy_enabled = false;
    config.tun_enabled = false;
    config.clash_api_enabled = true;
    tracing::info!(
        "Android forced override: disables MITM / system proxy / TUN (desktop semantics only, tun inbound injected in config composition); forces Clash API enabled (mandatory slice)"
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

/// PanelFeatures TUN toggle: Android traffic is taken over by VpnService — sing-box needs
/// config-level tun inbound to trigger libbox callback `openTun()` to establish VPN interface
/// (always true). Desktop passes through user settings as-is.
pub(crate) fn panel_features_tun_enabled(is_android: bool, tun_enabled: bool) -> bool {
    if is_android { true } else { tun_enabled }
}

/// PanelFeatures TUN `auto_route` toggle: Android's VpnService has no fallback system routing —
/// without `auto_route` the tun interface installs no routes and the tunnel is completely unusable,
/// so it is always forced on (same "always on" semantics as [`panel_features_tun_enabled`]).
/// Desktop passes through user settings as-is.
pub(crate) fn panel_features_tun_auto_route(is_android: bool, tun_auto_route: bool) -> bool {
    if is_android { true } else { tun_auto_route }
}

/// Calculate number of rules in composed config (sing-box `route.rules` array length,
/// missing array treated as 0).
pub(crate) fn config_json_rule_count(config_json: &serde_json::Value) -> u64 {
    config_json
        .get("route")
        .and_then(|r| r.get("rules"))
        .and_then(|rules| rules.as_array())
        .map_or(0, |rules| rules.len() as u64)
}
