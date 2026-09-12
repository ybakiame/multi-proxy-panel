use super::*;
use serde_json::json;

fn singbox_features() -> PanelFeatures {
    PanelFeatures {
        tun_enabled: true,
        tun_stack: "mixed".to_string(),
        tun_auto_route: true,
        clash_api_enabled: true,
        clash_api_port: 9090,
        clash_api_secret: "sekret".to_string(),
        clash_api_ui: "zashboard".to_string(),
        rule_mode: "rule".to_string(),
        ipv6_enabled: false,
        dns_fakeip_enabled: false,
        dns_mode: crate::config_slices::DnsMode::FollowSystem,
    }
}

fn fakeip_features(ipv6_enabled: bool) -> PanelFeatures {
    PanelFeatures {
        dns_fakeip_enabled: true,
        ipv6_enabled,
        ..singbox_features()
    }
}

/// Baseline subscription with an existing `local` DNS server (no route.rule_set).
fn base_sub() -> serde_json::Value {
    json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "rules": [],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    })
}

/// FakeIP on (ipv6 off): fakeip server appended, DNS rules `[drop, fakeip]` (the CN split —
/// `clash_mode` + `geosite-cn → local` — is owned by the baseline and is absent from this
/// baseline-less subscription), the CN-split remote rule sets registered, `experimental.cache_file`
/// deep-merged on, `dns.final` / `dns.strategy` untouched by fakeip (strategy still `ipv4_only`
/// from the IPv6 switch), sibling `clash_api` preserved, and **no** `resolve` route rule injected.
#[test]
fn apply_fakeip_mode_injects_fakeip_rules_and_rule_set() {
    let mut cfg = compose_singbox_config(&base_sub(), 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(false));

    // fakeip server appended (idempotent tag).
    let servers = cfg["dns"]["servers"].as_array().unwrap();
    let fakeip = servers
        .iter()
        .find(|s| s["tag"] == "fakeip")
        .expect("fakeip server should be injected");
    assert_eq!(fakeip["type"], "fakeip");
    assert_eq!(fakeip["inet4_range"], "198.18.0.0/15");
    assert_eq!(servers.len(), 2, "existing local server preserved");

    // DNS rules: drop first (head), then non-CN A → fakeip (tail).
    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(
        rules[0],
        json!({ "query_type": ["HTTPS", "SVCB", "AAAA"], "action": "predefined", "rcode": "NOERROR" })
    );
    assert_eq!(
        rules[1],
        json!({
            "rule_set": ["geolocation-!cn"],
            "query_type": ["A"],
            "action": "route",
            "server": "fakeip"
        }),
        "only non-CN A queries enter fakeip (geolocation-!cn, GUI.for.SingBox semantics)"
    );
    assert_eq!(
        rules.len(),
        2,
        "no other DNS rules in this baseline-less subscription"
    );

    // CN-split remote rule sets registered for the core to download (idempotent registry).
    let rule_sets = cfg["route"]["rule_set"].as_array().unwrap();
    assert_eq!(rule_sets.len(), 5);
    let geolocation = rule_sets
        .iter()
        .find(|rs| rs["tag"] == "geolocation-!cn")
        .expect("geolocation-!cn rule set must be registered");
    assert_eq!(
        geolocation["url"],
        "https://testingcf.jsdelivr.net/gh/MetaCubeX/meta-rules-dat@sing/geo/geosite/geolocation-!cn.srs"
    );
    assert!(
        rule_sets.iter().any(|rs| rs["tag"] == "geosite-cn"),
        "geosite-cn registered by the shared baseline registry"
    );

    // cache_file deep merge + clash_api sibling preserved.
    assert_eq!(cfg["experimental"]["cache_file"]["enabled"], true);
    assert_eq!(cfg["experimental"]["cache_file"]["store_fakeip"], true);
    assert_eq!(
        cfg["experimental"]["clash_api"]["external_controller"],
        "127.0.0.1:9090"
    );

    // fakeip does not touch final / strategy (strategy owned by the IPv6 switch).
    assert_eq!(cfg["dns"]["final"], "local");
    assert_eq!(cfg["dns"]["strategy"], "ipv4_only");

    // No global resolve rule: FakeIP now carries the domain end-to-end to the outbound.
    let route_rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        route_rules.iter().all(|r| r["action"] != "resolve"),
        "global resolve rule must be removed: {route_rules:?}"
    );
}

/// FakeIP idempotency: a second `apply_panel_features` call must not duplicate the fakeip server,
/// the head DNS rules, the `geosite-cn` rule set or the cache_file keys.
#[test]
fn apply_fakeip_mode_is_idempotent() {
    let mut cfg = compose_singbox_config(&base_sub(), 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(false));
    let first = cfg.clone();
    apply_panel_features(&mut cfg, &fakeip_features(false));
    assert_eq!(
        cfg, first,
        "second fakeip injection must be a no-op (server/rules/rule_set/cache_file idempotent)"
    );
}

/// FakeIP on with `ipv6_enabled = true`: the predefined rule drops only HTTPS/SVCB (AAAA kept),
/// the strategy override leaves the template `prefer_ipv4` untouched, and no resolve rule appears.
#[test]
fn apply_fakeip_mode_keeps_aaaa_when_ipv6_enabled() {
    let mut cfg = compose_singbox_config(&base_sub(), 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(true));

    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(
        rules[0],
        json!({ "query_type": ["HTTPS", "SVCB"], "action": "predefined", "rcode": "NOERROR" }),
        "AAAA must be kept when ipv6_enabled = true"
    );
    assert_eq!(
        rules[1],
        json!({
            "rule_set": ["geolocation-!cn"],
            "query_type": ["A"],
            "action": "route",
            "server": "fakeip"
        })
    );
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");

    let route_rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        route_rules.iter().all(|r| r["action"] != "resolve"),
        "no resolve rule with ipv6 enabled either: {route_rules:?}"
    );
}

/// DNS slice takeover exempts the whole fakeip injection: no fakeip server, no injected DNS
/// rules, no cache_file, no `geosite-cn` rule set — the user's slice is left untouched.
#[test]
fn apply_fakeip_mode_skipped_on_takeover() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "user", "type": "udp", "server": "8.8.8.8", "server_port": 53 }],
            "rules": [{ "domain": "example.com", "action": "route", "server": "user" }],
            "final": "user",
            "strategy": "prefer_ipv6"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        dns_fakeip_enabled: true,
        dns_mode: crate::config_slices::DnsMode::Takeover,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let servers = cfg["dns"]["servers"].as_array().unwrap();
    assert!(
        servers.iter().all(|s| s["tag"] != "fakeip"),
        "takeover must skip fakeip server injection: {servers:?}"
    );
    assert_eq!(
        cfg["dns"]["rules"],
        json!([{ "domain": "example.com", "action": "route", "server": "user" }]),
        "takeover must leave the user DNS rules untouched"
    );
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv6");
    assert!(
        cfg["experimental"]
            .get("cache_file")
            .is_none_or(|v| v.is_null()),
        "takeover must not inject cache_file"
    );
    let route_rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        route_rules.iter().all(|r| r["action"] != "resolve"),
        "takeover must skip the fakeip resolve rule: {route_rules:?}"
    );
    assert!(
        cfg["route"]
            .get("rule_set")
            .and_then(|v| v.as_array())
            .is_none_or(|arr| arr.iter().all(|rs| rs["tag"] != "geosite-cn")),
        "takeover must skip the geosite-cn rule set injection: {cfg:?}"
    );
}

/// A pre-existing user/template `action = resolve` rule (explicit takeover of resolution) is left
/// untouched, and fakeip no longer adds a resolve rule of its own (count stays 1).
#[test]
fn apply_fakeip_mode_preserves_existing_resolve_rule() {
    let mut sub = base_sub();
    sub["route"] = json!({ "rules": [{ "action": "resolve", "strategy": "prefer_ipv6" }] });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(false));

    let route_rules = cfg["route"]["rules"].as_array().unwrap();
    let resolve_count = route_rules
        .iter()
        .filter(|r| r["action"] == "resolve")
        .count();
    assert_eq!(
        resolve_count, 1,
        "fakeip must not add a resolve rule: {route_rules:?}"
    );
    assert!(
        route_rules
            .iter()
            .any(|r| r == &json!({ "action": "resolve", "strategy": "prefer_ipv6" })),
        "user resolve rule must be left untouched: {route_rules:?}"
    );
}

/// An existing `route.rule_set` entry carrying the same `geosite-cn` tag (user/override supplied)
/// is respected: no duplicate and no overwrite (the other CN-split tags are still registered).
#[test]
fn apply_fakeip_mode_does_not_override_existing_cn_rule_set() {
    let mut sub = base_sub();
    sub["route"] = json!({
        "rule_set": [{
            "type": "local",
            "tag": "geosite-cn",
            "format": "source",
            "path": "/tmp/user-cn.json"
        }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(false));

    let rule_sets = cfg["route"]["rule_set"].as_array().unwrap();
    assert_eq!(rule_sets.len(), 5, "existing tag must not be duplicated");
    assert_eq!(
        rule_sets[0],
        json!({
            "type": "local",
            "tag": "geosite-cn",
            "format": "source",
            "path": "/tmp/user-cn.json"
        }),
        "user-supplied geosite-cn rule set must be left untouched"
    );
}

/// cache_file deep merge: existing keys (`path` / `cache_id`) are preserved, only `enabled` /
/// `store_fakeip` are forced on; `clash_api` sibling is still written by the settings layer.
#[test]
fn apply_fakeip_mode_merges_cache_file_without_clobbering() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "rules": [],
            "final": "local"
        },
        "experimental": {
            "cache_file": { "enabled": false, "path": "/tmp/pp-cache.db", "cache_id": "client-1" },
            "clash_api": { "external_controller": "0.0.0.0:60000", "secret": "old" }
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(false));

    let cache_file = &cfg["experimental"]["cache_file"];
    assert_eq!(cache_file["enabled"], true, "enabled forced on");
    assert_eq!(cache_file["store_fakeip"], true, "store_fakeip forced on");
    assert_eq!(cache_file["path"], "/tmp/pp-cache.db", "path preserved");
    assert_eq!(cache_file["cache_id"], "client-1", "cache_id preserved");
    // clash_api replaced wholesale by settings but still present (not clobbered by cache_file merge).
    assert_eq!(
        cfg["experimental"]["clash_api"]["external_controller"],
        "127.0.0.1:9090"
    );
}

/// FakeIP off: no fakeip server, no injected DNS rules, no cache_file, no `geosite-cn` rule set
/// (isolated with `ipv6_enabled = true` so the IPv6 strategy override does not confound).
#[test]
fn apply_fakeip_mode_disabled_leaves_dns_untouched() {
    let mut cfg = compose_singbox_config(&base_sub(), 17890, None).unwrap();
    let features = PanelFeatures {
        dns_fakeip_enabled: false,
        ipv6_enabled: true,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let servers = cfg["dns"]["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 1, "no fakeip server injected");
    assert!(servers.iter().all(|s| s["tag"] != "fakeip"));
    assert_eq!(cfg["dns"]["rules"], json!([]));
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
    assert!(
        cfg["experimental"]
            .get("cache_file")
            .is_none_or(|v| v.is_null()),
        "disabled fakeip must not inject cache_file"
    );
    assert!(
        cfg["route"]
            .get("rule_set")
            .and_then(|v| v.as_array())
            .is_none_or(|arr| arr.iter().all(|rs| rs["tag"] != "geosite-cn")),
        "disabled fakeip must not inject the geosite-cn rule set"
    );
    let route_rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        route_rules.iter().all(|r| r["action"] != "resolve"),
        "disabled fakeip must not inject the resolve rule: {route_rules:?}"
    );
}

/// No `dns` object -> fakeip does not fabricate one (nor the cache_file / rule set), aligning with
/// the existing defensive style.
#[test]
fn apply_fakeip_mode_requires_existing_dns() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    assert!(cfg.get("dns").is_none(), "baseline has no dns object");
    apply_panel_features(&mut cfg, &fakeip_features(false));
    assert!(
        cfg.get("dns").is_none(),
        "fakeip must not fabricate a dns object"
    );
    assert!(
        cfg["experimental"]
            .get("cache_file")
            .is_none_or(|v| v.is_null()),
        "no dns -> fakeip injection skipped entirely, no cache_file"
    );
    assert!(
        cfg["route"]
            .get("rule_set")
            .and_then(|v| v.as_array())
            .is_none_or(|arr| arr.iter().all(|rs| rs["tag"] != "geosite-cn")),
        "no dns -> fakeip injection skipped entirely, no rule set"
    );
    let route_rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        route_rules.iter().all(|r| r["action"] != "resolve"),
        "no dns -> fakeip injection skipped entirely, no resolve rule: {route_rules:?}"
    );
}
