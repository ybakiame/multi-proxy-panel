//! CN-split baseline: the remote rule-set registry plus the DNS / route rules that make up the
//! GUI.for.SingBox-aligned default chain.
//!
//! The baseline is injected by [`crate::profile::singbox_template`]; the FakeIP injection
//! ([`super::fakeip::apply_fakeip_mode`]) reuses [`ensure_cn_rule_sets`] idempotently.

use serde_json::{Value, json};

use crate::config_slices::{
    DnsMatchType, DnsRule, DnsRuleAction, DnsServer, DnsServerType, DnsSlice, DnsStrategy,
};

/// The built-in (default) DNS configuration as an editable [`DnsSlice`] — the same shape the
/// running core actually uses, mapped into the slice schema so the frontend DNS editor can
/// display and modify it directly instead of treating the built-in DNS as an opaque black box
/// (the "edit the defaults ⇒ takeover" flow, ADR-0005 2026-09 补记).
///
/// Composition mirrors the runtime layering exactly:
///
/// - `servers` = template / `inject_android_dns` pair: `local` (DoH 223.5.5.5:443, direct) and
///   `remote` (DoH 8.8.8.8:443, `detour` = main selector tag `proxy`);
/// - `rules` = the panel-feature drop rule ([`super::fakeip::drop_query_types`]: HTTPS/SVCB,
///   plus AAAA when `ipv6_enabled = false`) at the head, then
///   [`cn_baseline_dns_rules`] (clash_mode direct/global → local/remote, geosite-cn → local);
/// - `final_tag` = `remote`, `strategy` = `prefer_ipv4` (IPv6 on) / `ipv4_only` (IPv6 off,
///   mirroring the panel-feature strategy override), `reverse_mapping` = on.
///
/// FakeIP is deliberately **not** part of this view: it is an opt-in panel feature with its
/// own settings switch, layered on top of this baseline.
///
/// Rule `id`s are stable `builtin-*` strings (not UUIDs) so the frontend can recognize the
/// unmodified default rows; materializing the view as a takeover body keeps them harmless.
#[must_use]
pub fn builtin_dns_slice(ipv6_enabled: bool) -> DnsSlice {
    let mut drop_target = "HTTPS,SVCB".to_string();
    if !ipv6_enabled {
        drop_target.push_str(",AAAA");
    }
    let rule = |id: &str, match_type: DnsMatchType, target: &str, server_tag: &str| DnsRule {
        id: id.to_string(),
        enabled: true,
        match_type,
        target: target.to_string(),
        server_tag: server_tag.to_string(),
        action: DnsRuleAction::Route,
        rcode: String::new(),
    };
    DnsSlice {
        mode: Default::default(),
        servers: vec![
            DnsServer {
                tag: "local".to_string(),
                server: "223.5.5.5".to_string(),
                server_type: DnsServerType::Https,
                server_port: Some(443),
                ..Default::default()
            },
            DnsServer {
                tag: "remote".to_string(),
                server: "8.8.8.8".to_string(),
                server_type: DnsServerType::Https,
                server_port: Some(443),
                detour: OUTBOUND_TAG_PROXY.to_string(),
                ..Default::default()
            },
        ],
        rules: vec![
            DnsRule {
                id: "builtin-drop".to_string(),
                enabled: true,
                match_type: DnsMatchType::QueryType,
                target: drop_target,
                server_tag: String::new(),
                action: DnsRuleAction::Predefined,
                rcode: "NOERROR".to_string(),
            },
            rule(
                "builtin-mode-direct",
                DnsMatchType::ClashMode,
                "direct",
                "local",
            ),
            rule(
                "builtin-mode-global",
                DnsMatchType::ClashMode,
                "global",
                "remote",
            ),
            rule(
                "builtin-geosite-cn",
                DnsMatchType::RuleSet,
                "geosite-cn",
                "local",
            ),
        ],
        final_tag: "remote".to_string(),
        strategy: if ipv6_enabled {
            DnsStrategy::PreferIpv4
        } else {
            DnsStrategy::Ipv4Only
        },
        reverse_mapping: true,
    }
}
///
/// HTTP client tag used for remote rule-set downloads (direct dial, bootstrap-resolved).
///
/// Registered at the top-level `http_clients` array by [`ensure_rule_set_http_client`] and
/// referenced by every CN-split rule set registered in [`ensure_cn_rule_sets`].
pub const RULE_SET_HTTP_CLIENT_TAG: &str = "rule-set-direct";

/// CN-split remote rule-set registry (`tag`, jsDelivr URL), aligned with the GUI.for.SingBox
/// default profile and the MetaCubeX `meta-rules-dat@sing` source family already used by the
/// FakeIP split.
///
/// `geolocation-!cn` lives under the `geosite/` directory (it is a geosite list of non-CN
/// domains); the rest map to their same-named `geosite/` / `geoip/` entry.
pub const CN_RULE_SETS: [(&str, &str); 5] = [
    (
        "geosite-private",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/private.srs",
    ),
    (
        "geoip-private",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/private.srs",
    ),
    (
        "geosite-cn",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/cn.srs",
    ),
    (
        "geoip-cn",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geoip/cn.srs",
    ),
    (
        "geolocation-!cn",
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/geolocation-!cn.srs",
    ),
];

/// Built-in outbound tags (single source of truth shared by [`crate::profile::singbox_template`]
/// and the read-only baseline view [`crate::core_config::BaselineView`]).
pub const OUTBOUND_TAG_PROXY: &str = "proxy";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_AUTO: &str = "auto";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_DIRECT: &str = "direct";
/// See [`OUTBOUND_TAG_PROXY`].
pub const OUTBOUND_TAG_BLOCK: &str = "block";

/// Built-in outbound `type` values (single source of truth shared by
/// [`crate::profile::singbox_template`] and [`crate::core_config::BaselineView`]).
pub const OUTBOUND_KIND_SELECTOR: &str = "selector";
/// See [`OUTBOUND_KIND_SELECTOR`].
pub const OUTBOUND_KIND_URLTEST: &str = "urltest";
/// See [`OUTBOUND_KIND_SELECTOR`].
pub const OUTBOUND_KIND_DIRECT: &str = "direct";
/// See [`OUTBOUND_KIND_SELECTOR`].
pub const OUTBOUND_KIND_BLOCK: &str = "block";

/// CN-split baseline DNS rules (GUI.for.SingBox default profile): `clash_mode direct → local`,
/// `clash_mode global → remote`, `geosite-cn → local`; `dns.final = remote` handles the rest.
///
/// The clash_mode rules make DNS follow the outbound mode switch (direct mode resolves
/// everything through the domestic resolver, global through the remote one) and deliberately
/// precede the FakeIP rule, so FakeIP never leaks into direct/global mode.
pub fn cn_baseline_dns_rules() -> Vec<Value> {
    vec![
        json!({ "clash_mode": "direct", "action": "route", "server": "local" }),
        json!({ "clash_mode": "global", "action": "route", "server": "remote" }),
        json!({ "rule_set": ["geosite-cn"], "action": "route", "server": "local" }),
    ]
}

/// CN-split baseline route rules (GUI.for.SingBox default profile): private / CN destinations
/// go `direct`, `geolocation-!cn` (non-CN) goes through the main selector.
///
/// `proxy_tag` is the main selector outbound tag, read from the generated outbounds rather than
/// hardcoded so the rules follow the template's actual group tag.
pub fn cn_baseline_route_rules(proxy_tag: &str) -> Vec<Value> {
    vec![
        json!({ "rule_set": ["geosite-private"], "outbound": "direct" }),
        json!({ "rule_set": ["geosite-cn"], "outbound": "direct" }),
        json!({ "rule_set": ["geoip-private"], "outbound": "direct" }),
        json!({ "rule_set": ["geoip-cn"], "outbound": "direct" }),
        json!({ "rule_set": ["geolocation-!cn"], "outbound": proxy_tag }),
    ]
}

/// Register the CN-split remote rule sets under `route.rule_set` (idempotent by tag).
///
/// Used by [`crate::profile::singbox_template`] (baseline) and by the FakeIP injection
/// ([`super::fakeip::apply_fakeip_mode`]) so a rule referencing `geosite-cn` /
/// `geolocation-!cn` always resolves. Existing entries with the same tag (user / override
/// supplied) are respected and left untouched.
///
/// Note: sing-box downloads remote rule sets synchronously at startup and **fails to start**
/// when a URL is unreachable (verified against sing-box 1.14). Registered entries reference the
/// [`RULE_SET_HTTP_CLIENT_TAG`] HTTP client so downloads dial **directly** (resolved via the
/// direct `local` DNS) instead of being routed through `route.final` (the proxy) — otherwise a
/// cold start would depend on the proxy being up before any rule can even be evaluated (and the
/// proxy itself is what those rules route). `experimental.cache_file` (injected by the panel
/// features stage, see `super::fakeip::ensure_cache_file`) is the offline fallback on top.
pub fn ensure_cn_rule_sets(route: &mut serde_json::Map<String, Value>) {
    let rule_set = route
        .entry("rule_set")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = rule_set.as_array_mut() else {
        return;
    };
    for (tag, url) in CN_RULE_SETS {
        if arr
            .iter()
            .any(|rs| rs.get("tag").and_then(Value::as_str) == Some(tag))
        {
            continue;
        }
        arr.push(json!({
            "type": "remote",
            "tag": tag,
            "format": "binary",
            "url": url,
            "http_client": RULE_SET_HTTP_CLIENT_TAG
        }));
    }
}

/// Ensure the top-level `http_clients` array carries the [`RULE_SET_HTTP_CLIENT_TAG`] client:
/// direct dial (no `detour` — HTTP clients bypass the routing system unless one is given) with
/// the download URL's domain resolved by the direct `local` DNS server, so remote rule-set
/// downloads never depend on the proxy path (see [`ensure_cn_rule_sets`]).
///
/// No-op when the config has no `local` DNS server (a fully user-owned DNS body, where
/// hardcoding a resolver reference would risk an invalid config); existing entries with the
/// same tag (user-supplied) are respected.
pub fn ensure_rule_set_http_client(obj: &mut serde_json::Map<String, Value>) {
    let has_local_dns = obj
        .get("dns")
        .and_then(|d| d.get("servers"))
        .and_then(Value::as_array)
        .is_some_and(|servers| {
            servers
                .iter()
                .any(|s| s.get("tag").and_then(Value::as_str) == Some("local"))
        });
    if !has_local_dns {
        tracing::debug!("无 local DNS server，跳过规则集直连 http_client 注入");
        return;
    }
    let clients = obj
        .entry("http_clients")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = clients.as_array_mut() else {
        return;
    };
    if arr
        .iter()
        .any(|c| c.get("tag").and_then(Value::as_str) == Some(RULE_SET_HTTP_CLIENT_TAG))
    {
        return;
    }
    arr.push(json!({
        "tag": RULE_SET_HTTP_CLIENT_TAG,
        "domain_resolver": { "server": "local" }
    }));
}
