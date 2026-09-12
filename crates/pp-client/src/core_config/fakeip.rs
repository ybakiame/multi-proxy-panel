//! FakeIP mode injection (opt-in) for the sing-box built-in DNS.
//!
//! # Architecture: FakeIP as CN bypass
//!
//! FakeIP is only meaningful for **non-CN** destinations: their domain must reach the proxy
//! outbound end-to-end (the core must route by domain, not by a locally resolved real IP), while
//! CN destinations must resolve through the local (direct) resolver so they connect to real,
//! reachable IPs.
//!
//! The CN split itself is owned by the baseline ([`super::cn_baseline_dns_rules`]): the
//! `geosite-cn → local` rule already sends CN domains to the domestic resolver and the
//! `clash_mode` rules already pin direct/global mode. FakeIP only adds two rules on top:
//!
//! 1. `HTTPS` / `SVCB` (and `AAAA` when IPv6 is off) queries are answered `NOERROR` with no
//!    records — they must never leak to a resolver (head rule). Since the drop rule is now
//!    injected unconditionally for every DNS mode (see [`inject_dns_drop_rule`], called from
//!    `apply_singbox_panel_features`), this is normally a no-op safety net here;
//! 2. **non-CN A queries** (`rule_set = geolocation-!cn`, `query_type = A`) are routed to the
//!    `fakeip` server, so the proxy outbound receives the **domain** (end-to-end) instead of a
//!    fake/real IP (tail rule, after the baseline so `clash_mode` direct/global wins and FakeIP
//!    never leaks into those modes).
//!
//! `geolocation-!cn` is the geosite list of non-CN domains, the same rule set the route baseline
//! already references. It is deliberately preferred over the previous `geosite-cn` + `invert`
//! shape: the explicit non-CN list matches the GUI.for.SingBox default semantics and keeps AAAA
//! out of FakeIP (IPv6 off drops it above; IPv6 on falls through to `dns.final` for a real
//! answer — the `fakeip` server only declares `inet4_range`).
//!
//! The rule set is registered by the CN-split baseline ([`super::ensure_cn_rule_sets`], called
//! here too as an idempotent safety net).
//!
//! **Startup caveat**: sing-box downloads remote rule sets synchronously at startup and fails to
//! start when a URL is unreachable and no cached copy exists (verified against sing-box 1.14).
//! The earlier "download failure degrades gracefully" note was incorrect — the core never reaches
//! rule evaluation. The jsDelivr mirror is directly reachable in CN. Since 1.14 the deprecated
//! `store_rdrc` / `store_dns` are replaced by automatic remote rule-set caching into
//! `experimental.cache_file` (bucket `rule_set`): once a rule set has been downloaded
//! successfully, its content is restored from the cache on the next start and the unreachable-URL
//! startup failure is avoided — which is exactly why FakeIP pins an explicit persistent cache
//! path below. The whole feature is opt-in and skipped entirely on DNS slice takeover (ADR-0005 D1).
//!
//! Note on the removed global `resolve` route rule: earlier revisions injected
//! `{"action":"resolve"}` into `route.rules` because FakeIP destinations had to be resolved to a
//! real IP before sing-box would route them to an outbound. That defeated the FakeIP purpose
//! (the outbound saw a real IP, not the domain) and made every connection depend on a
//! proxied DoH (`dns.final = remote`). With the CN split above, non-CN A queries resolve to a
//! FakeIP, the route rule set matches the sniffed domain, and the proxy outbound receives the
//! domain directly — no `resolve` rule is injected at all.

use std::path::Path;

use serde_json::{Value, json};

use super::PanelFeatures;

/// Rule-set tag for the non-CN FakeIP split (the CN-split baseline registers it).
const NON_CN_RULE_SET_TAG: &str = "geolocation-!cn";

/// FakeIP mode injection (opt-in; caller skips entirely on DNS takeover).
///
/// Operates on an already-existing `dns` object (never fabricates one, aligning with the IPv6
/// strategy override / `inject_android_dns` defensive style):
///
/// 1. append a `fakeip` server (`{"type":"fakeip","tag":"fakeip","inet4_range":"198.18.0.0/15"}`)
///    when no server tagged `fakeip` exists (idempotent);
/// 2. inject the two FakeIP DNS rules (idempotent, exact match per rule):
///    - `{"query_type":["HTTPS","SVCB"(,"AAAA")],"action":"predefined","rcode":"NOERROR"}` —
///      inserted at the head so records that must not be answered are always dropped (`AAAA`
///      only when `ipv6_enabled = false`);
///    - `{"rule_set":["geolocation-!cn"],"query_type":["A"],"action":"route","server":"fakeip"}`
///      — appended after the baseline rules so `clash_mode` direct/global and `geosite-cn` win;
///      only non-CN A queries enter FakeIP, so the proxy outbound receives the domain
///      end-to-end. `HTTPS`/`SVCB`/`AAAA` are excluded by query type / the drop rule; non-CN
///      AAAA (IPv6 on) falls through to `dns.final` for a real answer;
/// 3. register the CN-split rule sets in `route.rule_set` (idempotent by tag, reuses
///    [`super::ensure_cn_rule_sets`]; the baseline already registers `geolocation-!cn` /
///    `geosite-cn`, so this is normally a no-op);
/// 4. deep-merge `experimental.cache_file`: force `enabled = true` and `store_fakeip = true`
///    (preserving every other key and sibling `experimental` keys such as `clash_api`), and
///    fill `path` with an explicit absolute `<data_dir>/cache.db` when no non-empty `path` is
///    already present. A bare `cache.db` default is resolved by libbox against the
///    platform-dependent working path, so the explicit path keeps the FakeIP mapping (and the
///    sing-box 1.14 remote rule-set cache) in the client's persistent data directory across
///    core restarts; a user-supplied `path` (config slice / template) is preserved as-is, as
///    is `cache_id`.
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

    let fakeip_rule = json!({
        "rule_set": [NON_CN_RULE_SET_TAG],
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

        // 2. FakeIP DNS rules (idempotent, exact match per rule). The drop rule normally
        //    already exists (injected unconditionally by `apply_singbox_panel_features`,
        //    see [`inject_dns_drop_rule`]); the call here is the safety net for callers
        //    that bypass the panel-features stage.
        inject_dns_drop_rule(dns, features.ipv6_enabled);
        let rules = dns
            .entry("rules")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(arr) = rules.as_array_mut() {
            let has_fakeip = arr.iter().any(|r| {
                r.get("action").and_then(Value::as_str) == Some("route")
                    && r.get("server").and_then(Value::as_str) == Some("fakeip")
                    && rule_set_references(r, NON_CN_RULE_SET_TAG)
                    && query_type_set_matches(r, &fakeip_rule)
            });
            // FakeIP is appended after the baseline so `clash_mode` direct/global and
            // `geosite-cn` win.
            if !has_fakeip {
                arr.push(fakeip_rule);
            }
        }
    }

    // 3. Register the CN-split rule sets (idempotent by tag; baseline normally already did).
    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    if let Some(route_obj) = route.as_object_mut() {
        super::ensure_cn_rule_sets(route_obj);
    }

    // 4. experimental.cache_file deep merge (create the object when missing; sibling
    // experimental keys such as clash_api stay untouched). `enabled` / `store_fakeip` are
    // forced on; `path` is pinned to the client's persistent `<data_dir>/cache.db` only when
    // absent/empty (a user-supplied path and `cache_id` are preserved), so FakeIP mappings
    // survive a core restart independent of the platform working directory.
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
            let has_explicit_path = cf
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|p| !p.is_empty());
            if !has_explicit_path && !features.data_dir.is_empty() {
                let path = Path::new(&features.data_dir).join("cache.db");
                cf.insert(
                    "path".to_string(),
                    Value::String(path.to_string_lossy().into_owned()),
                );
            }
        }
    }
}

/// Drop-rule query types: `HTTPS` / `SVCB` always; `AAAA` too when IPv6 is off (the third
/// v6 defense layer suppresses AAAA, see [`super::ipv6`]).
pub(super) fn drop_query_types(ipv6_enabled: bool) -> Vec<Value> {
    let mut types = vec![
        Value::String("HTTPS".to_string()),
        Value::String("SVCB".to_string()),
    ];
    if !ipv6_enabled {
        types.push(Value::String("AAAA".to_string()));
    }
    types
}

/// Inject the `predefined NOERROR` drop rule at the head of `dns.rules` (idempotent).
///
/// `HTTPS` / `SVCB` (and `AAAA` when IPv6 is off) queries are answered `NOERROR` with no
/// records instead of being forwarded to a resolver: they carry endpoint/ECH hints sing-box
/// never consumes, and letting them fall through to `dns.final` (`remote`, dialed through the
/// proxy) would add a proxied round trip to Android's frequent HTTPS-type queries — or stall
/// name resolution entirely when the proxy path is down. Both reference templates
/// (`platforms/android/realip.json` / `fakeip.json`) carry this as the first DNS rule.
///
/// Head position: the drop must precede every routing rule (including the `clash_mode`
/// baseline and the FakeIP route rule) so dropped types never leak into any mode.
pub(super) fn inject_dns_drop_rule(dns: &mut serde_json::Map<String, Value>, ipv6_enabled: bool) {
    let drop_rule = json!({
        "query_type": drop_query_types(ipv6_enabled),
        "action": "predefined",
        "rcode": "NOERROR"
    });
    let rules = dns
        .entry("rules")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = rules.as_array_mut() else {
        return;
    };
    let has_drop = arr.iter().any(|r| {
        r.get("action").and_then(Value::as_str) == Some("predefined")
            && r.get("rcode").and_then(Value::as_str) == Some("NOERROR")
            && query_type_set_matches(r, &drop_rule)
    });
    if !has_drop {
        arr.insert(0, drop_rule);
    }
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
