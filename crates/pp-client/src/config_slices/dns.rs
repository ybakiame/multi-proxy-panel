//! DNS slice schema (ADR-0005 §3.1).
//!
//! [`crate::config_slices::render_dns`] renders this into the sing-box 1.12+
//! type-based DNS format (each `dns.servers` entry carries `type`).
//!
//! Supported capabilities:
//! - server types `udp` / `tls` / `https` / `quic` / `h3` / `local` / `fakeip`
//!   (fakeip carries `inet4_range` / `inet6_range` instead of `server`)
//! - match types `domain` / `domain_suffix` / `domain_keyword` / `rule_set` /
//!   `query_type` (comma-separated target, rendered as an array)
//! - rule actions `route` (default) / `predefined` (`rcode`) / `reject`

use serde::{Deserialize, Serialize};

use super::default_true;

/// Default FakeIP IPv4 range emitted when a fakeip server leaves `inet4_range`
/// empty (sing-box default, matching the reference `fakeip.json` template).
pub const DEFAULT_FAKEIP_INET4_RANGE: &str = "198.18.0.0/15";

/// DNS query type names accepted by the `query_type` match field.
///
/// Matching is case-insensitive; [`normalize_query_type`] uppercases the value
/// before rendering (sing-box expects the canonical uppercase form).
const QUERY_TYPE_NAMES: &[&str] = &[
    "A", "NS", "CNAME", "SOA", "PTR", "MX", "TXT", "AAAA", "SRV", "NAPTR", "CAA", "TLSA", "DS",
    "DNSKEY", "RRSIG", "NSEC", "NSEC3", "SVCB", "HTTPS", "ANY", "OPT", "HINFO", "MINFO", "WKS",
    "AXFR", "IXFR",
];

/// DNS response codes accepted by the `predefined` action's `rcode` field.
const DNS_RCODE_NAMES: &[&str] = &[
    "NOERROR", "FORMERR", "SERVFAIL", "NXDOMAIN", "NOTIMP", "REFUSED",
];

/// Whether `value` is a known DNS query type name (case-insensitive).
#[must_use]
pub fn is_valid_query_type(value: &str) -> bool {
    let normalized = value.trim().to_ascii_uppercase();
    QUERY_TYPE_NAMES.contains(&normalized.as_str())
}

/// Canonical (uppercase, trimmed) form of a query type name.
#[must_use]
pub fn normalize_query_type(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

/// Whether `value` is a known DNS response code (case-insensitive).
#[must_use]
pub fn is_valid_rcode(value: &str) -> bool {
    let normalized = value.trim().to_ascii_uppercase();
    DNS_RCODE_NAMES.contains(&normalized.as_str())
}

/// Canonical (uppercase, trimmed) form of a response code.
#[must_use]
pub fn normalize_rcode(value: &str) -> String {
    value.trim().to_ascii_uppercase()
}

/// DNS slice: structured form of the sing-box top-level `dns` object.
///
/// All fields are `#[serde(default)]` for forward compatibility; a missing or
/// partial object deserializes to sensible defaults (slice disabled).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsSlice {
    /// Slice master switch; `false` never injects the DNS slice.
    #[serde(default)]
    pub enabled: bool,
    /// Platform DNS mode (only Android diverges; desktop always behaves as takeover).
    #[serde(default)]
    pub mode: DnsMode,
    /// DNS servers (curated fields).
    #[serde(default)]
    pub servers: Vec<DnsServer>,
    /// DNS routing rules (curated fields).
    #[serde(default)]
    pub rules: Vec<DnsRule>,
    /// Default server tag (sing-box `dns.final`); required when takeover is on.
    #[serde(default)]
    pub final_tag: String,
    /// Default domain strategy (sing-box `dns.strategy`).
    #[serde(default)]
    pub strategy: DnsStrategy,
}

/// Platform DNS mode (ADR-0005 D1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsMode {
    /// Follow the system resolver: Android keeps `inject_android_dns`, so the
    /// slice DNS body does not take effect.
    #[default]
    FollowSystem,
    /// Take over DNS: `inject_android_dns` is skipped and the slice body applies.
    Takeover,
}

/// Domain resolution strategy (maps to sing-box `strategy` / `domain_strategy`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsStrategy {
    #[default]
    PreferIpv4,
    PreferIpv6,
    Ipv4Only,
    Ipv6Only,
}

impl DnsStrategy {
    /// sing-box wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreferIpv4 => "prefer_ipv4",
            Self::PreferIpv6 => "prefer_ipv6",
            Self::Ipv4Only => "ipv4_only",
            Self::Ipv6Only => "ipv6_only",
        }
    }
}

/// A single DNS server (curated fields).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsServer {
    /// Unique tag within the slice; referenced by `dns.rules` and `dns.final`.
    #[serde(default)]
    pub tag: String,
    /// Server address (IP or domain); unused for [`DnsServerType::Local`] and
    /// [`DnsServerType::Fakeip`].
    #[serde(default)]
    pub server: String,
    /// Server type (`udp` / `tls` / `https` / `quic` / `h3` / `local` / `fakeip`).
    #[serde(default)]
    pub server_type: DnsServerType,
    /// Server port (sing-box default per type when omitted).
    #[serde(default)]
    pub server_port: Option<u16>,
    /// FakeIP IPv4 range (`fakeip` type only); empty renders
    /// [`DEFAULT_FAKEIP_INET4_RANGE`].
    #[serde(default)]
    pub inet4_range: String,
    /// FakeIP IPv6 range (`fakeip` type only); omitted when empty.
    #[serde(default)]
    pub inet6_range: String,
    /// Dial field: upstream outbound tag (empty = default direct dial).
    #[serde(default)]
    pub detour: String,
    /// Per-server strategy override.
    ///
    /// **Not rendered**: sing-box 1.12+ new DNS servers have no per-server query
    /// strategy field (the old `strategy` server field was removed). The top-level
    /// [`DnsSlice::strategy`] and DNS rule actions cover this instead; the field
    /// is kept for schema completeness / future use.
    #[serde(default)]
    pub strategy: Option<DnsStrategy>,
    /// Dial field: DNS server tag used to resolve this server's own domain.
    #[serde(default)]
    pub domain_resolver: String,
}

/// DNS server type (`dns.servers[].type`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsServerType {
    #[default]
    Udp,
    Tls,
    Https,
    Quic,
    H3,
    /// System resolver; rendered without `server` / `server_port`.
    Local,
    /// FakeIP resolver; rendered with `inet4_range` / `inet6_range` instead of
    /// `server` / `server_port` / `detour`.
    Fakeip,
}

impl DnsServerType {
    /// sing-box wire value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Udp => "udp",
            Self::Tls => "tls",
            Self::Https => "https",
            Self::Quic => "quic",
            Self::H3 => "h3",
            Self::Local => "local",
            Self::Fakeip => "fakeip",
        }
    }

    /// Whether this type dials a `server` address.
    ///
    /// `false` for [`Self::Local`] and [`Self::Fakeip`], which render no
    /// `server` field and do not require one in validation.
    #[must_use]
    pub const fn uses_server(self) -> bool {
        !matches!(self, Self::Local | Self::Fakeip)
    }
}

/// A single DNS routing rule (rendered to `dns.rules[]`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnsRule {
    /// Unique rule ID (UUID v4 generated by the frontend).
    #[serde(default)]
    pub id: String,
    /// Whether the rule is active.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Match type (determines the sing-box match field).
    #[serde(default)]
    pub match_type: DnsMatchType,
    /// Match target (semantics depend on `match_type`).
    ///
    /// For [`DnsMatchType::QueryType`] this is a comma-separated list (e.g.
    /// `A,AAAA`).
    #[serde(default)]
    pub target: String,
    /// Target DNS server tag (`action: "route"`).
    #[serde(default)]
    pub server_tag: String,
    /// Rule action; absent in older data deserializes to [`DnsRuleAction::Route`].
    #[serde(default)]
    pub action: DnsRuleAction,
    /// Response code for [`DnsRuleAction::Predefined`] (empty = `NOERROR`).
    #[serde(default)]
    pub rcode: String,
}

/// DNS rule action (`dns.rules[].action`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsRuleAction {
    /// Route the query to [`DnsRule::server_tag`] (sing-box default).
    #[default]
    Route,
    /// Respond with a predefined `rcode` (sing-box 1.12+).
    Predefined,
    /// Reject the query.
    Reject,
}

/// DNS rule match type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DnsMatchType {
    #[default]
    Domain,
    DomainSuffix,
    DomainKeyword,
    RuleSet,
    /// DNS query type (`query_type`); target is a comma-separated list.
    QueryType,
}

impl DnsMatchType {
    /// sing-box match field name for this match type.
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::DomainSuffix => "domain_suffix",
            Self::DomainKeyword => "domain_keyword",
            Self::RuleSet => "rule_set",
            Self::QueryType => "query_type",
        }
    }
}
