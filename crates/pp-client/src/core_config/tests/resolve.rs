//! RealIP-mode route `resolve` action rule tests (see `core_config::singbox::inject_resolve_rule`).

use super::features::singbox_features;
use super::*;
use serde_json::json;

fn plain_sub() -> serde_json::Value {
    json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": { "final": "direct" }
    })
}

/// RealIP mode (FakeIP off): `{"action":"resolve"}` is injected right after hijack-dns so
/// domain-target connections (mixed inbound) can match the `geoip-cn` / `geoip-private` IP
/// rule sets. `strategy = ipv4_only` mirrors the IPv6 switch; the rule idempotently survives
/// a second application.
#[test]
fn apply_singbox_panel_features_injects_resolve_rule_in_realip_mode() {
    let mut cfg = compose_singbox_config(&plain_sub(), 17890, None).unwrap();
    let features = PanelFeatures {
        dns_fakeip_enabled: false,
        ipv6_enabled: false,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let rules = cfg["route"]["rules"].as_array().unwrap();
    let hijack_at = rules
        .iter()
        .position(|r| r["action"] == "hijack-dns")
        .expect("hijack-dns rule present");
    assert_eq!(
        rules[hijack_at + 1],
        json!({ "action": "resolve", "strategy": "ipv4_only" }),
        "resolve lands right after hijack-dns with ipv4_only when IPv6 is off"
    );

    // Idempotent: second application is a no-op.
    let before = cfg.clone();
    apply_panel_features(&mut cfg, &features);
    assert_eq!(cfg, before, "second injection must be a no-op");
}

/// `ipv6_enabled = true`: the resolve rule carries no `strategy` (AAAA stays resolvable).
#[test]
fn apply_singbox_panel_features_resolve_rule_omits_strategy_when_ipv6_enabled() {
    let mut cfg = compose_singbox_config(&plain_sub(), 17890, None).unwrap();
    let features = PanelFeatures {
        dns_fakeip_enabled: false,
        ipv6_enabled: true,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let rules = cfg["route"]["rules"].as_array().unwrap();
    let resolve = rules
        .iter()
        .find(|r| r["action"] == "resolve")
        .expect("resolve rule present");
    assert!(
        resolve.get("strategy").is_none(),
        "IPv6 on -> resolve keeps the DNS default strategy: {resolve}"
    );
}

/// FakeIP mode exempt: destinations must reach the proxy outbound as domains end-to-end, so no
/// resolve rule is injected.
#[test]
fn apply_singbox_panel_features_no_resolve_rule_in_fakeip_mode() {
    let mut cfg = compose_singbox_config(&plain_sub(), 17890, None).unwrap();
    let features = PanelFeatures {
        dns_fakeip_enabled: true,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        rules.iter().all(|r| r["action"] != "resolve"),
        "fakeip mode must not inject the resolve rule: {rules:?}"
    );
}

/// DNS slice takeover exempt: the user owns the DNS/route interplay (ADR-0005 D1).
#[test]
fn apply_singbox_panel_features_no_resolve_rule_on_dns_takeover() {
    let mut cfg = compose_singbox_config(&plain_sub(), 17890, None).unwrap();
    let features = PanelFeatures {
        dns_mode: crate::config_slices::DnsMode::Takeover,
        ..singbox_features()
    };
    apply_panel_features(&mut cfg, &features);

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert!(
        rules.iter().all(|r| r["action"] != "resolve"),
        "takeover -> no resolve rule injected: {rules:?}"
    );
}

/// A user/template-supplied `resolve` rule is respected: the injection is skipped and the
/// user's rule keeps its position.
#[test]
fn apply_singbox_panel_features_resolve_rule_respects_user_override() {
    let sub = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "route": {
            "final": "direct",
            "rules": [{ "action": "resolve", "strategy": "prefer_ipv4" }]
        }
    });
    let mut cfg = compose_singbox_config(&sub, 17890, None).unwrap();
    apply_panel_features(&mut cfg, &singbox_features());

    let rules = cfg["route"]["rules"].as_array().unwrap();
    assert_eq!(
        rules.iter().filter(|r| r["action"] == "resolve").count(),
        1,
        "user resolve rule not duplicated"
    );
    assert_eq!(
        rules.iter().find(|r| r["action"] == "resolve"),
        Some(&json!({ "action": "resolve", "strategy": "prefer_ipv4" })),
        "user's own resolve rule preserved as-is"
    );
}
