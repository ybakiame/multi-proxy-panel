//! Route slice schema (ADR-0005).
//!
//! Curated projection of the sing-box top-level `route` section, limited to
//! `final` (default outbound tag) and `default_domain_resolver` (sing-box 1.12+
//! dial field). Other route fields (`rules`, `rule_set`, `auto_detect_interface`,
//! …) stay owned by the template / ④ panel-feature layer and are intentionally
//! out of scope.
//!
//! Rendering lives in [`crate::config_slices::apply_config_slices`], which deep
//! merges the two keys into any existing `route` object so sibling keys
//! (`rules`, `auto_detect_interface`, …) survive.

use serde::{Deserialize, Serialize};

use super::dns::DnsStrategy;

/// Route slice: structured form of the sing-box `route` section.
///
/// All fields are `#[serde(default)]`; a missing or partial object deserializes
/// to sensible defaults (no overrides). No master switch: a non-empty
/// `final_tag` / `resolver.server` is written, empty fields are left untouched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSlice {
    /// `route.final` override (default outbound tag). Empty = keep the template
    /// `"proxy"` value.
    #[serde(default)]
    pub final_tag: String,
    /// `route.default_domain_resolver` override.
    #[serde(default)]
    pub resolver: DomainResolverSlice,
}

/// sing-box `route.default_domain_resolver` (curated fields).
///
/// Since sing-box 1.12.0 this is a dial-field object
/// `{ server, strategy?, client_subnet? }` (the legacy bare-string form is still
/// accepted upstream but never emitted here). `client_subnet` is out of scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainResolverSlice {
    /// DNS server tag used to resolve outbound domains. Empty = no override.
    #[serde(default)]
    pub server: String,
    /// Optional resolution strategy (sing-box `strategy`); `None` omits the key.
    #[serde(default)]
    pub strategy: Option<DnsStrategy>,
}

#[cfg(test)]
#[path = "tests/route_tests.rs"]
mod tests;
