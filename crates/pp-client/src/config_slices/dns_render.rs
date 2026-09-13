//! DNS slice rendering (sing-box 1.12+ type-based format).
//!
//! Split out of [`super::apply`] to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use serde_json::{Map, Value};

use super::dns::{
    DEFAULT_FAKEIP_INET4_RANGE, DnsMatchType, DnsRule, DnsRuleAction, DnsServer, DnsServerType,
    normalize_query_type, normalize_rcode,
};
use super::{DnsSlice, str_value};

/// Render a [`DnsSlice`] as a sing-box 1.12+ type-based DNS object.
///
/// Every server entry carries `type`; [`DnsServerType::Local`] entries omit
/// `server` / `server_port`, and [`DnsServerType::Fakeip`] entries render
/// `inet4_range` / `inet6_range` instead of dial fields. The top-level
/// `strategy` is always emitted.
#[must_use]
pub fn render_dns(dns: &DnsSlice) -> Value {
    // Disabled (弃用) servers are kept in the slice for reference but never rendered.
    let servers: Vec<Value> = dns
        .servers
        .iter()
        .filter(|server| server.enabled)
        .map(render_dns_server)
        .collect();
    let rules: Vec<Value> = dns
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(render_dns_rule)
        .collect();

    let mut out = Map::new();
    out.insert("servers".to_string(), Value::Array(servers));
    out.insert("rules".to_string(), Value::Array(rules));
    if !dns.final_tag.is_empty() {
        out.insert("final".to_string(), str_value(&dns.final_tag));
    }
    out.insert("strategy".to_string(), str_value(dns.strategy.as_str()));
    if dns.reverse_mapping {
        out.insert("reverse_mapping".to_string(), Value::Bool(true));
    }
    Value::Object(out)
}

/// Render a single DNS server entry (sing-box 1.12+ new format).
///
/// `fakeip` servers emit `inet4_range` (defaulting to
/// [`DEFAULT_FAKEIP_INET4_RANGE`]) / `inet6_range` and no dial fields; every
/// other type emits `server` / `server_port` / `detour` and never the FakeIP
/// ranges.
fn render_dns_server(server: &DnsServer) -> Value {
    let mut out = Map::new();
    out.insert("tag".to_string(), str_value(&server.tag));
    out.insert("type".to_string(), str_value(server.server_type.as_str()));

    if server.server_type == DnsServerType::Fakeip {
        let inet4 = if server.inet4_range.trim().is_empty() {
            DEFAULT_FAKEIP_INET4_RANGE
        } else {
            server.inet4_range.as_str()
        };
        out.insert("inet4_range".to_string(), str_value(inet4));
        if !server.inet6_range.trim().is_empty() {
            out.insert("inet6_range".to_string(), str_value(&server.inet6_range));
        }
        return Value::Object(out);
    }

    if server.server_type.uses_server() {
        out.insert("server".to_string(), str_value(&server.server));
        if let Some(port) = server.server_port {
            out.insert("server_port".to_string(), Value::from(port));
        }
    }
    if !server.detour.is_empty() {
        out.insert("detour".to_string(), str_value(&server.detour));
    }
    if !server.domain_resolver.is_empty() {
        out.insert(
            "domain_resolver".to_string(),
            str_value(&server.domain_resolver),
        );
    }
    Value::Object(out)
}

/// Render a single DNS rule match value.
///
/// [`DnsMatchType::QueryType`] targets are comma-separated and rendered as an
/// array of canonical uppercase names; [`DnsMatchType::RuleSet`] targets are
/// comma-separated tag lists (sing-box `rule_set` accepts an array, enabling
/// one DNS rule to match multiple rule sets) rendered as a trimmed non-empty
/// array, falling back to a single-element array when no comma is present;
/// [`DnsMatchType::ClashMode`] renders the mode as a bare string (sing-box
/// expects `clash_mode` as a string, not an array); all other match types
/// render a single-element array.
fn render_dns_match(rule: &DnsRule) -> Value {
    if rule.match_type.renders_string_target() {
        str_value(&rule.target)
    } else if rule.match_type == DnsMatchType::QueryType {
        Value::Array(
            rule.target
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(|item| str_value(&normalize_query_type(item)))
                .collect(),
        )
    } else if rule.match_type == DnsMatchType::RuleSet && rule.target.contains(',') {
        Value::Array(
            rule.target
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str_value)
                .collect(),
        )
    } else {
        Value::Array(vec![str_value(&rule.target)])
    }
}

/// Render a single DNS rule with its action.
///
/// `route` (the default) emits `"action":"route"` + the `server` tag;
/// `predefined` emits `"action":"predefined"` + `rcode` (defaulting to
/// `NOERROR`); `reject` emits `"action":"reject"` only.
fn render_dns_rule(rule: &DnsRule) -> Value {
    let mut out = Map::new();
    out.insert(rule.match_type.field().to_string(), render_dns_match(rule));

    match rule.action {
        DnsRuleAction::Route => {
            out.insert("action".to_string(), str_value("route"));
            out.insert("server".to_string(), str_value(&rule.server_tag));
        }
        DnsRuleAction::Predefined => {
            out.insert("action".to_string(), str_value("predefined"));
            let rcode = if rule.rcode.trim().is_empty() {
                "NOERROR".to_string()
            } else {
                normalize_rcode(&rule.rcode)
            };
            out.insert("rcode".to_string(), str_value(&rcode));
        }
        DnsRuleAction::Reject => {
            out.insert("action".to_string(), str_value("reject"));
        }
    }
    Value::Object(out)
}
