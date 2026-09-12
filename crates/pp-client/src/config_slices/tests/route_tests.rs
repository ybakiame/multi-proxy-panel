//! Route slice tests (schema / validation / render + merge / resolver interaction).
//!
//! Kept in a dedicated file (one per slice domain) so the source modules stay
//! within the business-file size gate (`.agents/rules/code-organization.md`).

use crate::config_slices::*;
use serde_json::json;

fn slices_with_route(route: RouteSlice) -> ConfigSlices {
    ConfigSlices {
        route,
        ..Default::default()
    }
}

#[test]
fn route_default_is_disabled_with_no_overrides() {
    let route = RouteSlice::default();
    assert!(!route.enabled);
    assert!(route.final_tag.is_empty());
    assert!(route.resolver.server.is_empty());
    assert!(route.resolver.strategy.is_none());
}

#[test]
fn route_missing_fields_fall_back_to_defaults() {
    let route: RouteSlice = serde_json::from_str("{}").unwrap();
    assert_eq!(route, RouteSlice::default());

    let empty: RouteSlice = serde_json::from_str(r#"{"enabled":true,"resolver":{}}"#).unwrap();
    assert!(empty.enabled);
    assert!(empty.resolver.server.is_empty());
    assert!(empty.resolver.strategy.is_none());
}

#[test]
fn config_slices_roundtrip_includes_route() {
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: "direct".to_string(),
        resolver: DomainResolverSlice {
            server: "remote".to_string(),
            strategy: Some(DnsStrategy::Ipv4Only),
        },
    });
    let text = serde_json::to_string(&slices).unwrap();
    let back: ConfigSlices = serde_json::from_str(&text).unwrap();
    assert_eq!(slices, back);
}

#[test]
fn validate_accepts_enabled_slice() {
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: "proxy".to_string(),
        resolver: DomainResolverSlice {
            server: "remote".to_string(),
            strategy: Some(DnsStrategy::PreferIpv4),
        },
    });
    slices.validate().unwrap();
}

#[test]
fn validate_accepts_unknown_resolver_tag() {
    // Tag reference integrity is a runtime concern (subscription node / built-in
    // `local` / `remote` / `fakeip` tag), not a static slice check.
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: String::new(),
        resolver: DomainResolverSlice {
            server: "some-subscription-tag".to_string(),
            strategy: None,
        },
    });
    slices.validate().unwrap();
}

#[test]
fn validate_rejects_whitespace_final_and_resolver() {
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: "bad tag".to_string(),
        ..Default::default()
    });
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("route.final"), "{err}");

    let slices = slices_with_route(RouteSlice {
        enabled: true,
        resolver: DomainResolverSlice {
            server: "bad tag".to_string(),
            strategy: None,
        },
        ..Default::default()
    });
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("default_domain_resolver"), "{err}");
}

#[test]
fn validate_skips_disabled_slice() {
    let slices = slices_with_route(RouteSlice {
        enabled: false,
        final_tag: "bad tag".to_string(),
        resolver: DomainResolverSlice {
            server: "bad tag".to_string(),
            strategy: None,
        },
    });
    slices.validate().unwrap();
}

#[test]
fn render_domain_resolver_omits_absent_strategy() {
    let value = render_domain_resolver(&DomainResolverSlice {
        server: "local".to_string(),
        strategy: None,
    });
    assert_eq!(value, json!({ "server": "local" }));

    let value = render_domain_resolver(&DomainResolverSlice {
        server: "remote".to_string(),
        strategy: Some(DnsStrategy::Ipv6Only),
    });
    assert_eq!(
        value,
        json!({ "server": "remote", "strategy": "ipv6_only" })
    );
}

#[test]
fn apply_overrides_final_and_resolver_independently() {
    // final only: resolver and sibling keys stay untouched.
    let mut config = json!({
        "route": {
            "rules": [],
            "final": "proxy",
            "auto_detect_interface": true,
            "default_domain_resolver": { "server": "local" }
        }
    });
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: "direct".to_string(),
        resolver: DomainResolverSlice::default(),
    });
    apply_config_slices(&mut config, &slices, true).unwrap();
    assert_eq!(config["route"]["final"], "direct");
    assert_eq!(
        config["route"]["default_domain_resolver"]["server"],
        "local"
    );
    assert_eq!(config["route"]["auto_detect_interface"], true);
    assert_eq!(config["route"]["rules"], json!([]));

    // resolver only: final and sibling keys stay untouched.
    let mut config = json!({
        "route": { "final": "proxy", "default_domain_resolver": { "server": "local" } }
    });
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: String::new(),
        resolver: DomainResolverSlice {
            server: "remote".to_string(),
            strategy: Some(DnsStrategy::Ipv4Only),
        },
    });
    apply_config_slices(&mut config, &slices, true).unwrap();
    assert_eq!(config["route"]["final"], "proxy");
    assert_eq!(
        config["route"]["default_domain_resolver"],
        json!({ "server": "remote", "strategy": "ipv4_only" })
    );
}

#[test]
fn apply_creates_route_object_when_absent() {
    let mut config = json!({ "outbounds": [] });
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: "proxy".to_string(),
        resolver: DomainResolverSlice {
            server: "local".to_string(),
            strategy: None,
        },
    });
    apply_config_slices(&mut config, &slices, true).unwrap();
    assert_eq!(config["route"]["final"], "proxy");
    assert_eq!(
        config["route"]["default_domain_resolver"],
        json!({ "server": "local" })
    );
}

#[test]
fn apply_gate_off_skips_route_slice() {
    let mut config = json!({
        "route": { "final": "proxy", "default_domain_resolver": { "server": "local" } }
    });
    let original = config.clone();
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: "direct".to_string(),
        resolver: DomainResolverSlice {
            server: "remote".to_string(),
            strategy: None,
        },
    });
    apply_config_slices(&mut config, &slices, false).unwrap();
    assert_eq!(config, original);
}

#[test]
fn apply_disabled_slice_leaves_config_unchanged() {
    let mut config = json!({ "route": { "final": "proxy" } });
    let original = config.clone();
    let slices = slices_with_route(RouteSlice {
        enabled: false,
        final_tag: "direct".to_string(),
        ..Default::default()
    });
    apply_config_slices(&mut config, &slices, true).unwrap();
    assert_eq!(config, original);
}

/// The ⓪ slice layer runs before ④ `compose_singbox_config`, whose
/// `ensure_domain_resolver` only fills the resolver when absent. A
/// slice-written resolver must therefore survive composition unchanged.
#[test]
fn slice_resolver_survives_ensure_domain_resolver() {
    // DNS body declares `local`; the slice route resolver points at `remote`.
    let mut config = json!({
        "outbounds": [{ "type": "direct", "tag": "direct" }],
        "dns": { "servers": [{ "tag": "local", "type": "udp", "server": "223.5.5.5" }] },
        "route": { "final": "proxy", "default_domain_resolver": { "server": "local" } }
    });
    let slices = slices_with_route(RouteSlice {
        enabled: true,
        final_tag: String::new(),
        resolver: DomainResolverSlice {
            server: "remote".to_string(),
            strategy: None,
        },
    });
    apply_config_slices(&mut config, &slices, true).unwrap();
    assert_eq!(
        config["route"]["default_domain_resolver"]["server"],
        "remote"
    );

    // ④ compose must respect the pre-existing resolver (missing-only fill).
    let composed = crate::core_config::compose_singbox_config(&config, 17890, None).unwrap();
    assert_eq!(
        composed["route"]["default_domain_resolver"],
        json!({ "server": "remote" })
    );
    assert_eq!(composed["route"]["final"], "proxy");
}
