//! DNS query-type drop rule tests (every DNS mode, not only FakeIP; see
//! `core_config::fakeip::inject_dns_drop_rule`).

use super::features::singbox_features;
use super::*;
use serde_json::json;

// ---------- DNS query-type drop rule (every DNS mode, not only FakeIP) ----------

/// RealIP mode (FakeIP off): the `HTTPS`/`SVCB` (+`AAAA` when IPv6 off) predefined NOERROR
/// drop rule is injected at the head of `dns.rules`, so these query types are answered empty
/// locally instead of falling through to `dns.final` (remote, dialed through the proxy).
/// Existing rules keep their relative order after it.
#[test]
fn apply_singbox_panel_features_injects_dns_drop_rule_without_fakeip() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5" }],
            "rules": [{ "rule_set": ["geosite-cn"], "action": "route", "server": "local" }],
            "final": "local"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        dns_fakeip_enabled: false,
        ipv6_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let rules = cfg["dns"]["rules"].as_array().unwrap();
    assert_eq!(
        rules[0],
        json!({
            "query_type": ["HTTPS", "SVCB", "AAAA"],
            "action": "predefined",
            "rcode": "NOERROR"
        }),
        "drop rule must lead dns.rules (AAAA included when IPv6 is off)"
    );
    assert_eq!(
        rules[1],
        json!({ "rule_set": ["geosite-cn"], "action": "route", "server": "local" }),
        "existing rules keep their relative order after the drop rule"
    );
    assert!(
        cfg["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["tag"] != "fakeip"),
        "realip mode must not gain a fakeip server"
    );
}

/// `ipv6_enabled = true`: AAAA is answered for real, so the drop rule only covers
/// `HTTPS`/`SVCB`.
#[test]
fn apply_singbox_panel_features_dns_drop_rule_keeps_aaaa_when_ipv6_enabled() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5" }],
            "rules": [],
            "final": "local"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        ipv6_enabled: true,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    assert_eq!(
        cfg["dns"]["rules"][0],
        json!({
            "query_type": ["HTTPS", "SVCB"],
            "action": "predefined",
            "rcode": "NOERROR"
        }),
        "IPv6 on -> AAAA stays resolvable, only HTTPS/SVCB dropped"
    );
}

/// DNS slice takeover exempts the drop rule: the user owns the whole DNS body (ADR-0005 D1).
#[test]
fn apply_singbox_panel_features_dns_drop_rule_exempt_on_takeover() {
    let sub = json!({
        "dns": {
            "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5" }],
            "rules": [],
            "final": "local"
        },
        "outbounds": [{ "type": "direct", "tag": "direct" }]
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        dns_mode: crate::config_slices::DnsMode::Takeover,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    assert!(
        cfg["dns"]["rules"].as_array().unwrap().is_empty(),
        "takeover -> user DNS rules untouched, no drop rule injected"
    );
}
