//! FakeIP mode injection (opt-in) for the sing-box built-in DNS.
//!
//! # Architecture: FakeIP as CN bypass
//!
//! FakeIP is only meaningful for **non-CN** destinations: their domain must reach the proxy
//! outbound end-to-end (the core must route by domain, not by a locally resolved real IP), while
//! CN destinations must resolve through the local (direct) resolver so they connect to real,
//! reachable IPs. The two goals are satisfied by splitting the DNS rules on the `geosite-cn`
//! rule set:
//!
//! 1. `HTTPS` / `SVCB` (and `AAAA` when IPv6 is off) queries are answered `NOERROR` with no
//!    records — they must never leak to a resolver;
//! 2. **CN domains** (`rule_set = geosite-cn`) are routed to the `local` server for real
//!    resolution of every query type (AAAA, when present, is already dropped by rule 1);
//! 3. **non-CN A queries** (`rule_set = geosite-cn`, `invert = true`, `query_type = A`) are routed
//!    to the `fakeip` server, so the proxy outbound receives the **domain** (end-to-end) instead
//!    of a fake/real IP.
//!
//! Rule 3 is deliberately restricted to `A`: with `invert` the rule would otherwise also catch
//! AAAA queries for non-CN domains. Those are answered by rule 1 when IPv6 is off; when IPv6 is
//! on they fall through to `dns.final` for a real AAAA (no FakeIP for IPv6 — the `fakeip` server
//! only declares `inet4_range`).
//!
//! The `geosite-cn` rule set is registered as a **remote binary rule set** downloaded by the core
//! from jsDelivr (the same URL source family already used by the rule-set infrastructure, reached
//! directly in CN without a custom `http_client` / `download_detour`). Download failure degrades
//! gracefully: the two `geosite-cn` rules simply never match, so every A query falls through to
//! `dns.final` and gets a **real** resolution — connectivity is preserved (no FakeIP), never
//! broken. The whole feature is opt-in and skipped entirely on DNS slice takeover (ADR-0005 D1).
//!
//! Note on the removed global `resolve` route rule: earlier revisions injected
//! `{"action":"resolve"}` into `route.rules` because FakeIP destinations had to be resolved to a
//! real IP before sing-box would route them to an outbound. That defeated the FakeIP purpose
//! (the outbound saw a real IP, not the domain) and made every connection depend on a
//! proxied DoH (`dns.final = remote`). With the CN split above, non-CN A queries resolve to a
//! FakeIP, the route rule set matches the sniffed domain, and the proxy outbound receives the
//! domain directly — no `resolve` rule is injected at all.

use serde_json::{Value, json};

use super::PanelFeatures;

/// Rule-set tag used for the CN bypass split (matches the community `geosite-cn` convention).
const CN_RULE_SET_TAG: &str = "geosite-cn";

/// Remote binary rule-set URL for CN domains (same jsDelivr source family as the existing rule-set
/// infrastructure; reached directly in CN, so no custom download dialer is configured).
const CN_RULE_SET_URL: &str =
    "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/cn.srs";

/// FakeIP mode injection (opt-in; caller skips entirely on DNS takeover).
///
/// Operates on an already-existing `dns` object (never fabricates one, aligning with the IPv6
/// strategy override / `inject_android_dns` defensive style):
///
/// 1. append a `fakeip` server (`{"type":"fakeip","tag":"fakeip","inet4_range":"198.18.0.0/15"}`)
///    when no server tagged `fakeip` exists (idempotent);
/// 2. prepend three DNS rules (idempotent, exact match per rule), in head order
///    `[drop, cn-local, fakeip]` so they win over template/slice DNS rules:
///    - `{"query_type":["HTTPS","SVCB"(,"AAAA")],"action":"predefined","rcode":"NOERROR"}` — drops
///      records that must not be answered (`AAAA` only when `ipv6_enabled = false`);
///    - `{"rule_set":["geosite-cn"],"action":"route","server":"local"}` — CN domains resolve for
///      real through the direct/local server (every query type; AAAA already dropped above);
///    - `{"rule_set":["geosite-cn"],"invert":true,"query_type":["A"],"action":"route","server":
///      "fakeip"}` — only non-CN A queries enter FakeIP, so the proxy outbound receives the
///      domain end-to-end. `HTTPS`/`SVCB`/`AAAA` are excluded by query type / the drop rule;
///      non-CN AAAA (IPv6 on) falls through to `dns.final` for a real answer;
/// 3. register the `geosite-cn` remote rule set in `route.rule_set` (idempotent by tag; an
///    existing entry with the same tag — user/override supplied — is respected and left as-is);
/// 4. deep-merge `experimental.cache_file`: set only `enabled = true` and `store_fakeip = true`,
///    preserving every other key (and sibling `experimental` keys such as `clash_api`).
///
/// Does not touch `dns.final` / `dns.strategy` — the latter stays owned by the IPv6 override in
/// [`crate::core_config::apply_singbox_panel_features`]. `route.default_domain_resolver` is not
/// rewritten here: the fakeip server is appended, so the first-server tag chosen by
/// `ensure_domain_resolver` (or by `inject_android_dns`) is unchanged. No `route.rules` entry is
/// injected (the global `resolve` action was removed, see the module docs).
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
    let cn_local_rule = json!({
        "rule_set": [CN_RULE_SET_TAG],
        "action": "route",
        "server": "local"
    });
    let fakeip_rule = json!({
        "rule_set": [CN_RULE_SET_TAG],
        "invert": true,
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

        // 2. Head DNS rules (idempotent, exact match per rule).
        let rules = dns
            .entry("rules")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = rules.as_array_mut() {
            let has_drop = arr.iter().any(|r| {
                r.get("action").and_then(Value::as_str) == Some("predefined")
                    && r.get("rcode").and_then(Value::as_str) == Some("NOERROR")
                    && query_type_set_matches(r, &drop_rule)
            });
            let has_cn_local = arr.iter().any(|r| {
                r.get("action").and_then(Value::as_str) == Some("route")
                    && r.get("server").and_then(Value::as_str) == Some("local")
                    && rule_set_references(r, CN_RULE_SET_TAG)
            });
            let has_fakeip = arr.iter().any(|r| {
                r.get("action").and_then(Value::as_str) == Some("route")
                    && r.get("server").and_then(Value::as_str) == Some("fakeip")
                    && r.get("invert").and_then(Value::as_bool) == Some(true)
                    && rule_set_references(r, CN_RULE_SET_TAG)
                    && query_type_set_matches(r, &fakeip_rule)
            });
            // Insert fakeip first, then cn-local, then drop, so the final head order is
            // `[drop, cn-local, fakeip, ...original]`.
            if !has_fakeip {
                arr.insert(0, fakeip_rule);
            }
            if !has_cn_local {
                arr.insert(0, cn_local_rule);
            }
            if !has_drop {
                arr.insert(0, drop_rule);
            }
        }
    }

    // 3. Register the remote CN rule set (idempotent by tag; existing same-tag entry respected).
    inject_cn_rule_set(obj);

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

/// Register the `geosite-cn` remote binary rule set under `route.rule_set` (idempotent by tag).
///
/// A remote rule set is downloaded by the core itself. No `http_client` / `download_detour` is
/// configured: the jsDelivr mirror is directly reachable in CN, and the existing rule-set
/// infrastructure uses the same URL source family with the default direct dialer. Download
/// failure is non-fatal — the `geosite-cn` DNS rules never match, so queries fall through to
/// `dns.final` and are resolved for real (see the module docs).
///
/// Idempotency: when any entry in `route.rule_set` already carries `tag = geosite-cn` (second
/// call, or a user/override supplied rule set) the injection is skipped and the existing entry is
/// left untouched.
fn inject_cn_rule_set(obj: &mut serde_json::Map<String, Value>) {
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let rule_set = route_obj
        .entry("rule_set")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = rule_set.as_array_mut() else {
        return;
    };
    if arr
        .iter()
        .any(|rs| rs.get("tag").and_then(Value::as_str) == Some(CN_RULE_SET_TAG))
    {
        return;
    }
    arr.push(json!({
        "type": "remote",
        "tag": CN_RULE_SET_TAG,
        "format": "binary",
        "url": CN_RULE_SET_URL
    }));
}

/// Whether `rule`'s `rule_set` match field references `tag` (single-string or array form).
/// Used by [`apply_fakeip_mode`] for idempotency.
fn rule_set_references(rule: &Value, tag: &str) -> bool {
    match rule.get("rule_set") {
        Some(Value::String(s)) => s == tag,
        Some(Value::Array(arr)) => arr.iter().any(|v| v.as_str() == Some(tag)),
        _ => false,
    }
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
