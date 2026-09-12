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

/// FakeIP on (ipv6 off): fakeip server appended, head rules drop HTTPS/SVCB/AAAA + route A,
/// `experimental.cache_file` deep-merged on, `dns.final` / `dns.strategy` untouched by fakeip
/// (strategy still `ipv4_only` from the IPv6 switch), sibling `clash_api` preserved.
#[test]
fn apply_fakeip_mode_injects_server_rules_and_cache_file() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "rules": [],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
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

    // Head rules: drop first, then route A.
    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(
        rules[0],
        json!({ "query_type": ["HTTPS", "SVCB", "AAAA"], "action": "predefined", "rcode": "NOERROR" })
    );
    assert_eq!(
        rules[1],
        json!({ "query_type": ["A"], "action": "route", "server": "fakeip" })
    );
    assert_eq!(rules.len(), 2, "no other DNS rules in baseline");

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
}

/// FakeIP idempotency: a second `apply_panel_features` call must not duplicate the fakeip server,
/// the head DNS rules or the cache_file keys.
#[test]
fn apply_fakeip_mode_is_idempotent() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "rules": [],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(false));
    let first = cfg.clone();
    apply_panel_features(&mut cfg, &fakeip_features(false));
    assert_eq!(
        cfg, first,
        "second fakeip injection must be a no-op (server/rules/cache_file idempotent)"
    );
}

/// FakeIP on with `ipv6_enabled = true`: the predefined rule drops only HTTPS/SVCB (AAAA kept),
/// and the strategy override leaves the template `prefer_ipv4` untouched.
#[test]
fn apply_fakeip_mode_keeps_aaaa_when_ipv6_enabled() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "rules": [],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &fakeip_features(true));

    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(
        rules[0],
        json!({ "query_type": ["HTTPS", "SVCB"], "action": "predefined", "rcode": "NOERROR" }),
        "AAAA must be kept when ipv6_enabled = true"
    );
    assert_eq!(
        rules[1],
        json!({ "query_type": ["A"], "action": "route", "server": "fakeip" })
    );
    assert_eq!(cfg["dns"]["strategy"], "prefer_ipv4");
}

/// DNS slice takeover exempts the whole fakeip injection: no fakeip server, no injected DNS
/// rules, no cache_file — the user's slice DNS body is left untouched.
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

/// FakeIP off: no fakeip server, no injected DNS rules, no cache_file (isolated with
/// `ipv6_enabled = true` so the IPv6 strategy override does not confound the comparison).
#[test]
fn apply_fakeip_mode_disabled_leaves_dns_untouched() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5", "server_port": 53 }],
            "rules": [],
            "final": "local",
            "strategy": "prefer_ipv4"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
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
}

/// No `dns` object -> fakeip does not fabricate one (nor the cache_file), aligning with the
/// existing defensive style.
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
}
