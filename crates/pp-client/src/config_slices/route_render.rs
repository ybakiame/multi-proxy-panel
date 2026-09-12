//! Route slice rendering (sing-box 1.12+ `default_domain_resolver` object form).
//!
//! Split out of [`super::apply`] to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use serde_json::{Map, Value};

use super::{DomainResolverSlice, str_value};

/// Render a [`DomainResolverSlice`] as a sing-box `route.default_domain_resolver`
/// object.
///
/// `server` is always emitted (callers only invoke this when non-empty).
/// `strategy` is emitted only when set, so `None` leaves the key absent.
#[must_use]
pub fn render_domain_resolver(resolver: &DomainResolverSlice) -> Value {
    let mut out = Map::new();
    out.insert("server".to_string(), str_value(&resolver.server));
    if let Some(strategy) = resolver.strategy {
        out.insert("strategy".to_string(), str_value(strategy.as_str()));
    }
    Value::Object(out)
}
