//! Core config composition: synthesize subscription config into locally usable core startup config.

use std::net::SocketAddr;

use serde_json::Value;

mod clash_api;
mod compose;
mod mihomo;
mod singbox;

#[cfg(test)]
mod tests;

pub use clash_api::*;
pub use compose::*;
pub use mihomo::*;
pub use singbox::*;

/// MITM chain info: inbound / outbound / rules injected during core config synthesis.
pub struct MitmChain {
    /// MITM proxy listen address (target of the http outbound in core routing rules).
    pub proxy_addr: SocketAddr,
    /// MITM return inlet port (core return mixed inbound listen port, usually `mixed_port + 1`).
    pub return_port: u16,
    /// Hostnames to be MITM-whitelisted (`*.` prefix matched by suffix, others exact match);
    /// `-` / `!` prefix entries are exclusions, no core routing rules generated
    /// (corresponding domain traffic goes direct, not sent to MITM inbound).
    pub hostnames: Vec<String>,
}

/// Settings page TUN and Clash panel config.
///
/// These are "settings have highest priority" fields: injected by [`apply_panel_features`]
/// after `compose_*`, and template/override fields with the same name (tun inbound,
/// `experimental.clash_api`, `external-controller`) are replaced wholesale by settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelFeatures {
    /// Whether to enable TUN virtual network card (requires root/admin privileges).
    pub tun_enabled: bool,
    /// TUN protocol stack: `gvisor` / `system` / `mixed`.
    pub tun_stack: String,
    /// TUN auto route.
    pub tun_auto_route: bool,
    /// Whether to enable Clash panel API.
    pub clash_api_enabled: bool,
    /// Clash panel API listen port.
    pub clash_api_port: u16,
    /// Clash panel API secret (empty string = no auth, omitted in output).
    pub clash_api_secret: String,
    /// Clash panel UI choice: `yacd` / `zashboard` / `metacubexd` (unknown falls back to `zashboard`).
    pub clash_api_ui: String,
    /// Rule mode: `rule` / `global` / `direct` (invalid values fall back to `rule` at injection time).
    ///
    /// mihomo writes to top-level `mode:`; sing-box has no composition-level mode field, not written
    /// to config (runtime switched via Clash API `PATCH /configs`, see [`push_clash_mode`]).
    pub rule_mode: String,
}

/// Clash panel UI choice normalization: `yacd` / `zashboard` / `metacubexd` returned as-is,
/// others (including empty string) fall back to default panel `zashboard`.
///
/// Both cores' `external_ui` directory names (`ui-<choice>`) and download URLs are based on
/// the normalized result (see [`apply_singbox_panel_features`] / [`apply_mihomo_panel_features_impl`]).
fn normalized_clash_api_ui(ui: &str) -> &'static str {
    match ui {
        "yacd" => "yacd",
        "zashboard" => "zashboard",
        "metacubexd" => "metacubexd",
        _ => "zashboard",
    }
}

/// Clash panel UI download URL mapping (public convention, see Task R item 1).
///
/// Unknown values fall back to default panel `zashboard`.
pub fn clash_api_ui_download_url(ui: &str) -> &'static str {
    match normalized_clash_api_ui(ui) {
        "yacd" => "https://github.com/haishanh/yacd/archive/gh-pages.zip",
        "metacubexd" => "https://github.com/MetaCubeX/metacubexd/archive/gh-pages.zip",
        _ => "https://github.com/Zephyruso/zashboard/archive/gh-pages.zip",
    }
}

/// Apply settings page TUN / Clash panel config forced injection into already-composed core config.
///
/// Must be called after `compose_singbox_config` / `compose_mihomo_config` (i.e.
/// `build_core_config` + override overlay) to ensure settings have the highest priority.
pub fn apply_panel_features(
    composed: &mut Value,
    core_type: pp_common::CoreType,
    features: &PanelFeatures,
) {
    match core_type {
        pp_common::CoreType::SingBox => apply_singbox_panel_features(composed, features),
        pp_common::CoreType::Mihomo => apply_mihomo_panel_features(composed, features),
    }
}

/// Normalize rule mode: `rule` / `global` / `direct` returned as-is, others (including empty string)
/// fall back to `"rule"`.
pub(crate) fn normalized_rule_mode(mode: &str) -> &str {
    match mode {
        "rule" | "global" | "direct" => mode,
        _ => "rule",
    }
}
