//! Core config composition: synthesize subscription config into locally usable core startup config.

use std::net::SocketAddr;

use serde_json::Value;

use crate::config_slices::{ConfigSlices, DnsMode};

mod baseline;
mod clash_api;
mod compose;
mod fakeip;
mod singbox;
mod view;

#[cfg(test)]
mod tests;

pub use baseline::*;
pub use clash_api::*;
pub use compose::*;
pub use singbox::*;
pub use view::*;

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
/// `experimental.clash_api`) are replaced wholesale by settings.
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
    /// When Clash API is enabled, written into `experimental.clash_api.default_mode` (startup mode,
    /// normalized small-case) and backed by baseline `clash_mode` rules at `route.rules` head (see
    /// [`apply_singbox_panel_features`]); runtime switching via Clash API `PATCH /configs`
    /// ([`push_clash_mode`]) is kept as a secondary idempotent path.
    pub rule_mode: String,
    /// Whether to allow IPv6 resolution (DNS strategy switch).
    ///
    /// Default `false`: [`apply_singbox_panel_features`] rewrites the effective
    /// `dns.strategy` to `ipv4_only` so domains resolving to AAAA do not reach
    /// nodes without an IPv6 egress (connection failure / blackhole). `true`
    /// leaves the template/injected `prefer_ipv4` untouched. Exempt when
    /// `dns_mode` is [`DnsMode::Takeover`] (user DNS slice takes over fully).
    pub ipv6_enabled: bool,
    /// Whether to enable the built-in DNS FakeIP mode (opt-in, default `false`).
    ///
    /// When enabled, [`apply_singbox_panel_features`] appends a `fakeip` DNS server,
    /// routes all A queries to it, drops HTTPS/SVCB (and AAAA when `ipv6_enabled`
    /// is `false`) with `NOERROR`, and deep-merges `experimental.cache_file` to
    /// persist fakeip mappings. Exempt when `dns_mode` is [`DnsMode::Takeover`]
    /// (user DNS slice takes over fully).
    pub dns_fakeip_enabled: bool,
    /// Android DNS mode derived from [`crate::config_slices::ConfigSlices`] (ADR-0005 D1).
    ///
    /// `FollowSystem` (default) keeps the forced `inject_android_dns`; `Takeover`
    /// skips it so the config-slice DNS body (applied at the ⓪ layer) applies.
    /// Only meaningful on Android; desktop ignores it (the injection is Android-only).
    /// Derive with [`dns_mode_from_slices`].
    pub dns_mode: DnsMode,
    /// Client data directory (`ClientConfig::data_dir`), used by FakeIP mode to resolve the
    /// explicit persistent cache file path `<data_dir>/cache.db`.
    ///
    /// The sing-box `cache_file` default (bare `cache.db`, no explicit `path`) is resolved by
    /// libbox against the **working path** (`SetupOptions.workingPath`), which is
    /// platform-dependent (Android: `getExternalFilesDir`, desktop: process CWD / config dir).
    /// Passing an explicit absolute path pins the FakeIP mapping store — and the sing-box 1.14
    /// remote rule-set cache — to the client's own persistent data directory, independent of the
    /// platform working directory. Empty string leaves the sing-box default untouched.
    pub data_dir: String,
}

/// Clash panel UI choice normalization: `yacd` / `zashboard` / `metacubexd` returned as-is,
/// others (including empty string) fall back to default panel `zashboard`.
///
/// The `external_ui` directory names (`ui-<choice>`) and download URLs are based on
/// the normalized result (see [`apply_singbox_panel_features`]).
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
/// Must be called after `compose_singbox_config` (i.e. `build_core_config` + override overlay)
/// to ensure settings have the highest priority.
pub fn apply_panel_features(composed: &mut Value, features: &PanelFeatures) {
    apply_singbox_panel_features(composed, features)
}

/// Derive the DNS mode from loaded config slices (ADR-0005 D1).
///
/// The DNS slice has no master switch: [`DnsMode::Takeover`] when the slice is
/// set to takeover, otherwise [`DnsMode::FollowSystem`]. This applies to both
/// Android and desktop; on Android `FollowSystem` keeps the forced
/// `inject_android_dns`, while on desktop the template DNS stays untouched.
#[must_use]
pub fn dns_mode_from_slices(slices: &ConfigSlices) -> DnsMode {
    slices.dns.mode
}
