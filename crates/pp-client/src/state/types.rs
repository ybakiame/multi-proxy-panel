//! Client runtime state types.

use std::net::SocketAddr;

/// Current client running status.
#[derive(Debug, Clone)]
pub struct ClientStatus {
    /// Whether the core is running.
    pub core_running: bool,
    /// MITM proxy listen address (`None` when not enabled).
    pub mitm_addr: Option<SocketAddr>,
    /// Whether system proxy is currently enabled.
    pub system_proxy: bool,
    /// Current effective rule mode (= persisted value in client.json, illegal values normalized to `rule`).
    pub rule_mode: String,
    /// Number of rules in the composed config (sing-box takes `route.rules`, mihomo takes `rules`
    /// array length; 0 when not running).
    pub rule_count: u64,
    /// Clash dashboard API address (when core is running and `clash_api_enabled`,
    /// `http://127.0.0.1:{clash_api_port}`, otherwise `None`).
    pub clash_api_url: Option<String>,
}
