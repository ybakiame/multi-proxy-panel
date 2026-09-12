//! `builtin_dns_slice` tests: the built-in default DNS mapped into the editable slice schema
//! (see `core_config::baseline::builtin_dns_slice`).

use super::*;
use crate::config_slices::{
    DnsMatchType, DnsMode, DnsRuleAction, DnsServerType, DnsStrategy, render_dns,
};
use serde_json::json;

/// IPv6 off (default): drop rule covers AAAA, strategy is `ipv4_only`; servers / rules /
/// final / reverse_mapping mirror the runtime layering (template servers + panel drop rule at
/// the head + baseline rules).
#[test]
fn builtin_dns_slice_ipv6_off() {
    let slice = builtin_dns_slice(false);

    assert_eq!(slice.mode, DnsMode::FollowSystem);
    let servers = &slice.servers;
    assert_eq!(servers.len(), 2);
    assert_eq!(servers[0].tag, "local");
    assert_eq!(servers[0].server_type, DnsServerType::Https);
    assert_eq!(servers[0].server, "223.5.5.5");
    assert_eq!(servers[0].server_port, Some(443));
    assert!(
        servers[0].detour.is_empty(),
        "local must stay detour-less (direct dial)"
    );
    assert_eq!(servers[1].tag, "remote");
    assert_eq!(servers[1].server_type, DnsServerType::Https);
    assert_eq!(servers[1].server, "8.8.8.8");
    assert_eq!(servers[1].detour, "proxy");

    let rules = &slice.rules;
    assert_eq!(rules.len(), 4);
    assert_eq!(rules[0].id, "builtin-drop");
    assert_eq!(rules[0].match_type, DnsMatchType::QueryType);
    assert_eq!(rules[0].target, "HTTPS,SVCB,AAAA");
    assert_eq!(rules[0].action, DnsRuleAction::Predefined);
    assert_eq!(rules[0].rcode, "NOERROR");
    assert_eq!(rules[1].match_type, DnsMatchType::ClashMode);
    assert_eq!(rules[1].target, "direct");
    assert_eq!(rules[1].server_tag, "local");
    assert_eq!(rules[2].match_type, DnsMatchType::ClashMode);
    assert_eq!(rules[2].target, "global");
    assert_eq!(rules[2].server_tag, "remote");
    assert_eq!(rules[3].match_type, DnsMatchType::RuleSet);
    assert_eq!(rules[3].target, "geosite-cn");
    assert_eq!(rules[3].server_tag, "local");

    assert_eq!(slice.final_tag, "remote");
    assert_eq!(slice.strategy, DnsStrategy::Ipv4Only);
    assert!(slice.reverse_mapping);
}

/// IPv6 on: AAAA is not dropped and the strategy stays `prefer_ipv4`.
#[test]
fn builtin_dns_slice_ipv6_on() {
    let slice = builtin_dns_slice(true);
    assert_eq!(slice.rules[0].target, "HTTPS,SVCB");
    assert_eq!(slice.strategy, DnsStrategy::PreferIpv4);
}

/// The slice validates against the slice schema (it is meant to be materializable as a
/// takeover body unchanged).
#[test]
fn builtin_dns_slice_passes_schema_validation() {
    builtin_dns_slice(false).validate().unwrap();
    builtin_dns_slice(true).validate().unwrap();
}

/// Rendering the builtin slice reproduces the exact runtime DNS shape: drop rule head with
/// query_type array, clash_mode rules as **string** match values, geosite-cn rule set array,
/// final/strategy/reverse_mapping.
#[test]
fn builtin_dns_slice_renders_runtime_shape() {
    let rendered = render_dns(&builtin_dns_slice(false));
    assert_eq!(
        rendered["rules"],
        json!([
            {
                "query_type": ["HTTPS", "SVCB", "AAAA"],
                "action": "predefined",
                "rcode": "NOERROR"
            },
            { "clash_mode": "direct", "action": "route", "server": "local" },
            { "clash_mode": "global", "action": "route", "server": "remote" },
            { "rule_set": ["geosite-cn"], "action": "route", "server": "local" }
        ]),
        "clash_mode must render as a string (not an array), drop rule at the head"
    );
    assert_eq!(rendered["final"], "remote");
    assert_eq!(rendered["strategy"], "ipv4_only");
    assert_eq!(rendered["reverse_mapping"], true);
    assert_eq!(
        rendered["servers"],
        json!([
            { "tag": "local", "type": "https", "server": "223.5.5.5", "server_port": 443 },
            {
                "tag": "remote",
                "type": "https",
                "server": "8.8.8.8",
                "server_port": 443,
                "detour": "proxy"
            }
        ])
    );
}

/// `reverse_mapping = false` renders nothing (field omitted, sing-box default applies).
#[test]
fn render_dns_omits_reverse_mapping_when_false() {
    let mut slice = builtin_dns_slice(false);
    slice.reverse_mapping = false;
    let rendered = render_dns(&slice);
    assert!(rendered.get("reverse_mapping").is_none());
}
