//! FakeIP mode injection (opt-in) for the sing-box built-in DNS.

use serde_json::{Value, json};

use super::PanelFeatures;

/// FakeIP mode injection (opt-in; caller skips entirely on DNS takeover).
///
/// Operates on an already-existing `dns` object (never fabricates one, aligning with the IPv6
/// strategy override / `inject_android_dns` defensive style):
///
/// 1. append a `fakeip` server (`{"type":"fakeip","tag":"fakeip","inet4_range":"198.18.0.0/15"}`)
///    when no server tagged `fakeip` exists (idempotent);
/// 2. prepend two DNS rules (idempotent, exact query-type-set + action match):
///    - `{"query_type":["HTTPS","SVCB"(,"AAAA")],"action":"predefined","rcode":"NOERROR"}` — drops
///      records that must not be answered (`AAAA` only when `ipv6_enabled = false`);
///    - `{"query_type":["A"],"action":"route","server":"fakeip"}` — all A queries resolve to fake
///      IPs. The two rules match disjoint query types, so their relative order is irrelevant; both
///      are inserted at the head so they win over template/slice DNS rules;
/// 3. inject a `{"action":"resolve"}` route rule (idempotent) right after the `hijack-dns` rule,
///    carrying `"strategy":"ipv4_only"` when `ipv6_enabled = false` (omitted otherwise) — FakeIP
///    destinations must be resolved to a real IP before sing-box will route them to
///    `direct`/outbound (otherwise: `a resolve action is required before routing to
///    outbound/direct[direct]`). See [`inject_resolve_route_rule`];
/// 4. deep-merge `experimental.cache_file`: set only `enabled = true` and `store_fakeip = true`,
///    preserving every other key (and sibling `experimental` keys such as `clash_api`).
///
/// Does not touch `dns.final` / `dns.strategy` — the latter stays owned by the IPv6 override in
/// [`crate::core_config::apply_singbox_panel_features`]. `route.default_domain_resolver` is not
/// rewritten here: the fakeip server is appended, so the first-server tag chosen by
/// `ensure_domain_resolver` (or by `inject_android_dns`) is unchanged.
pub(super) fn apply_fakeip_mode(composed: &mut Value, features: &PanelFeatures) {
    let Some(obj) = composed.as_object_mut() else {
        return;
    };

    let mut drop_query_types = vec![
        Value::String("HTTPS".to_string()),
        Value::String("SVCB".to_string()),
    ];
    if !features.ipv6_enabled {
        drop_query_types.push(Value::String("AAAA".to_string()));
    }
    let drop_rule = json!({
        "query_type": drop_query_types,
        "action": "predefined",
        "rcode": "NOERROR"
    });
    let route_rule = json!({
        "query_type": ["A"],
        "action": "route",
        "server": "fakeip"
    });

    {
        // Existing `dns` object required; never fabricate one.
        let Some(dns) = obj.get_mut("dns").and_then(Value::as_object_mut) else {
            return;
        };

        // 1. fakeip server (append; skip when tag already present).
        let servers = dns
            .entry("servers")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = servers.as_array_mut() {
            let has_fakeip = arr
                .iter()
                .any(|s| s.get("tag").and_then(Value::as_str) == Some("fakeip"));
            if !has_fakeip {
                arr.push(json!({
                    "type": "fakeip",
                    "tag": "fakeip",
                    "inet4_range": "198.18.0.0/15"
                }));
            }
        }

        // 2. Head DNS rules (idempotent by exact query-type set + action).
        let rules = dns
            .entry("rules")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = rules.as_array_mut() {
            let has_drop = arr.iter().any(|r| {
                r.get("action").and_then(Value::as_str) == Some("predefined")
                    && r.get("rcode").and_then(Value::as_str) == Some("NOERROR")
                    && query_type_set_matches(r, &drop_rule)
            });
            let has_route = arr.iter().any(|r| {
                r.get("action").and_then(Value::as_str) == Some("route")
                    && r.get("server").and_then(Value::as_str) == Some("fakeip")
                    && query_type_set_matches(r, &route_rule)
            });
            // Insert route first, then drop, so the final head order is
            // `[drop, route, ...original]`.
            if !has_route {
                arr.insert(0, route_rule);
            }
            if !has_drop {
                arr.insert(0, drop_rule);
            }
        }
    }

    // 3. FakeIP destinations require an explicit `resolve` route action before sing-box routes them
    // to `direct`/outbound; inserted after `hijack-dns` (see [`inject_resolve_route_rule`]). Only
    // reached when a `dns` object exists (i.e. fakeip was actually applied) — the DNS block above
    // early-returns otherwise, so a fakeip toggle without DNS never injects a dangling resolve rule.
    inject_resolve_route_rule(obj, features.ipv6_enabled);

    // 4. experimental.cache_file deep merge (create the object when missing; sibling
    // experimental keys such as clash_api stay untouched).
    let experimental = obj
        .entry("experimental")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(exp) = experimental.as_object_mut() {
        let cache_file = exp
            .entry("cache_file")
            .or_insert_with(|| Value::Object(Default::default()));
        if let Some(cf) = cache_file.as_object_mut() {
            cf.insert("enabled".to_string(), Value::Bool(true));
            cf.insert("store_fakeip".to_string(), Value::Bool(true));
        }
    }
}

/// Inject the `resolve` route action required by FakeIP destinations (idempotent).
///
/// FakeIP connections carry a fake destination address; sing-box refuses to route them to
/// `direct`/outbound until the domain is resolved into a real IP (`a resolve action is required
/// before routing to outbound/direct[direct]`). The reference template
/// (`sing-box-config-templates .../fakeip.json`) places `{"action":"resolve"}` right after the
/// sniff + hijack-dns rules, so it is inserted immediately after the `hijack-dns` rule (head when
/// none exists). `"strategy":"ipv4_only"` is added when IPv6 is disabled to mirror the DNS
/// strategy override; with IPv6 enabled the field is omitted (core default).
///
/// Idempotency: when `route.rules` already contains any `action = resolve` rule (second call, or a
/// user/template rule explicitly taking over resolution) the injection is skipped.
fn inject_resolve_route_rule(obj: &mut serde_json::Map<String, Value>, ipv6_enabled: bool) {
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let rules = route_obj
        .entry("rules")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(rules_arr) = rules.as_array_mut() else {
        return;
    };
    if rules_arr
        .iter()
        .any(|r| r.get("action").and_then(Value::as_str) == Some("resolve"))
    {
        return;
    }
    let mut resolve = serde_json::Map::new();
    resolve.insert("action".to_string(), Value::String("resolve".to_string()));
    if !ipv6_enabled {
        resolve.insert(
            "strategy".to_string(),
            Value::String("ipv4_only".to_string()),
        );
    }
    let at = rules_arr
        .iter()
        .position(|r| r.get("action").and_then(Value::as_str) == Some("hijack-dns"))
        .map_or(0, |i| i + 1);
    rules_arr.insert(at, Value::Object(resolve));
}

/// Whether `rule` carries a `query_type` array whose element set equals the corresponding field
/// of `expected` (order-insensitive). Used by [`apply_fakeip_mode`] for idempotency.
fn query_type_set_matches(rule: &Value, expected: &Value) -> bool {
    match (
        rule.get("query_type").and_then(Value::as_array),
        expected.get("query_type").and_then(Value::as_array),
    ) {
        (Some(actual), Some(expected)) => {
            actual.len() == expected.len() && actual.iter().all(|v| expected.contains(v))
        }
        _ => false,
    }
}
