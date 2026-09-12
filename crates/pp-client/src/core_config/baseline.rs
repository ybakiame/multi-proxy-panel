//! CN-split baseline: the remote rule-set registry plus the DNS / route rules that make up the
//! GUI.for.SingBox-aligned default chain.
//!
//! The baseline is injected by [`crate::profile::singbox_template`].

use serde_json::{Value, json};

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
/// Used by [`crate::profile::singbox_template`] so the baseline `geosite-cn` /
/// `geolocation-!cn` rule references always resolve. Existing entries with the same tag
/// (user / override supplied) are respected and left untouched.
///
/// Note: sing-box downloads remote rule sets synchronously at startup and **fails to start**
/// when a URL is unreachable (verified against sing-box 1.14). The jsDelivr mirror is directly
/// reachable in CN; `experimental.cache_file.store_dns` (or the deprecated `store_rdrc`) is the
/// only offline fallback and is not enabled by the baseline.
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
            "url": url
        }));
    }
}
