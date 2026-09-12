//! IPv6-off route-layer fast fail tests (see `core_config::ipv6`).

use super::features::singbox_features;
use super::*;
use serde_json::json;

/// `ipv6_enabled = false` (default): inject `{"ip_version": 6, "action": "reject"}` right after
/// `hijack-dns` and before the `clash_mode` baselines, so IPv6 connections that bypassed the
/// hijacked resolver (app HTTPDNS / DoH) fail fast inside the dual-stack TUN. This is the route
/// layer of the three-layer v6 defense (dual-stack TUN + `ipv4_only` DNS + route reject).
#[test]
fn apply_singbox_panel_features_injects_ipv6_reject_after_hijack_dns() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "final": "direct",
            "rules": [{ "domain": "sub.com", "outbound": "proxy" }]
        }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.iter().filter(|r| r["ip_version"] == 6).count(),
        1,
        "exactly one IPv6 reject rule"
    );
    assert_eq!(
        rules[2],
        json!({ "ip_version": 6, "action": "reject" }),
        "reject must sit after sniff/hijack-dns and before the clash_mode baselines"
    );
    // Full final order contract.
    assert_eq!(rules[0], json!({ "action": "sniff" }));
    assert_eq!(
        rules[1],
        json!({ "protocol": "dns", "action": "hijack-dns" })
    );
    assert_eq!(
        rules[3],
        json!({ "clash_mode": "direct", "outbound": "direct" })
    );
    assert_eq!(
        rules[4],
        json!({ "clash_mode": "global", "outbound": "proxy" })
    );
    assert_eq!(
        rules[5],
        json!({ "domain": "sub.com", "outbound": "proxy" })
    );
}

/// `ipv6_enabled = true`: no route reject rule is injected.
#[test]
fn apply_singbox_panel_features_no_ipv6_reject_when_enabled() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "final": "direct" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        ipv6_enabled: true,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        rules.iter().all(|r| r["ip_version"] != 6),
        "IPv6 on -> no reject rule: {rules:?}"
    );
}

/// Idempotency: a second `apply_panel_features` call must not duplicate the IPv6 reject rule.
#[test]
fn apply_singbox_panel_features_ipv6_reject_is_idempotent() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "final": "direct" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());
    let first = cfg.clone();
    apply_panel_features(&mut cfg, &singbox_features());
    assert_eq!(cfg, first, "second injection must be a no-op");

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.iter().filter(|r| r["ip_version"] == 6).count(),
        1,
        "no duplicate IPv6 reject rule"
    );
}

/// User override (Profile / template) with an `ip_version = 6` `reject` or `resolve` rule wins:
/// the injection is skipped so the user's explicit IPv6 handling is neither duplicated nor
/// shadowed by a prepended reject.
#[test]
fn apply_singbox_panel_features_ipv6_reject_skips_on_user_override() {
    for user_rule in [
        json!({ "ip_version": 6, "action": "reject" }),
        json!({ "ip_version": 6, "action": "resolve" }),
    ] {
        let sub = json!({
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": {
                "final": "direct",
                "rules": [user_rule.clone()]
            }
        });
        let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
        apply_panel_features(&mut cfg, &singbox_features());

        let rules = cfg["route"]["rules"].as_array().unwrap();
        assert_eq!(
            rules.iter().filter(|r| r["ip_version"] == 6).count(),
            1,
            "user override kept, no extra reject injected for {user_rule}"
        );
        assert_eq!(
            rules.iter().find(|r| r["ip_version"] == 6),
            Some(&user_rule),
            "the user's own rule is the single v6 rule"
        );
    }
}

/// DNS slice takeover does **not** exempt the route reject: it is a routing-layer guard,
/// independent of who owns DNS. The user's `dns.strategy` stays untouched (see the strategy
/// override test), but the in-tunnel v6 fast-fail is still injected.
#[test]
fn apply_singbox_panel_features_ipv6_reject_not_exempt_on_takeover() {
    let sub = json!({
        "dns": { "servers": [], "strategy": "prefer_ipv6" },
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "final": "direct" }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    let features = PanelFeatures {
        ipv6_enabled: false,
        dns_mode: crate::config_slices::DnsMode::Takeover,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    assert_eq!(
        cfg["dns"]["strategy"], "prefer_ipv6",
        "takeover keeps the user DNS strategy (DNS layer exempt)"
    );
    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        rules
            .iter()
            .any(|r| r["ip_version"] == 6 && r["action"] == "reject"),
        "takeover does NOT exempt the route-layer IPv6 reject: {rules:?}"
    );
}
