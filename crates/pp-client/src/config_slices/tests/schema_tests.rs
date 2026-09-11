//! Schema + validation tests for config slices.
//!
//! Split out of `schema.rs` to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use crate::config_slices::*;

fn dns_slice() -> DnsSlice {
    DnsSlice {
        enabled: true,
        mode: DnsMode::Takeover,
        servers: vec![
            DnsServer {
                tag: "local".to_string(),
                server: "223.5.5.5".to_string(),
                server_type: DnsServerType::Udp,
                server_port: Some(53),
                ..Default::default()
            },
            DnsServer {
                tag: "remote".to_string(),
                server: "8.8.8.8".to_string(),
                server_type: DnsServerType::Https,
                server_port: Some(443),
                detour: "proxy".to_string(),
                ..Default::default()
            },
        ],
        rules: vec![DnsRule {
            id: "r1".to_string(),
            enabled: true,
            match_type: DnsMatchType::DomainSuffix,
            target: ".cn".to_string(),
            server_tag: "local".to_string(),
        }],
        final_tag: "remote".to_string(),
        strategy: DnsStrategy::PreferIpv4,
    }
}

fn outbounds_slice() -> OutboundsSlice {
    OutboundsSlice {
        enabled: true,
        items: vec![
            CustomOutbound {
                id: "o1".to_string(),
                name: "My Vless".to_string(),
                enabled: true,
                protocol: OutboundProtocol::Vless(VlessOutbound {
                    server: "example.com".to_string(),
                    server_port: 443,
                    uuid: "uuid-1".to_string(),
                    flow: "xtls-rprx-vision".to_string(),
                    tls: OutboundTls {
                        enabled: true,
                        server_name: "example.com".to_string(),
                        ..Default::default()
                    },
                    transport: OutboundTransport {
                        kind: "ws".to_string(),
                        path: "/ws".to_string(),
                        host: "example.com".to_string(),
                    },
                }),
            },
            CustomOutbound {
                id: "o2".to_string(),
                name: "ss node".to_string(),
                enabled: true,
                protocol: OutboundProtocol::Shadowsocks(ShadowsocksOutbound {
                    server: "1.2.3.4".to_string(),
                    server_port: 8388,
                    method: "aes-256-gcm".to_string(),
                    password: "secret".to_string(),
                }),
            },
        ],
    }
}

fn sample_slices() -> ConfigSlices {
    ConfigSlices {
        version: SLICE_VERSION,
        dns: dns_slice(),
        outbounds: outbounds_slice(),
    }
}

#[test]
fn default_has_version_and_disabled_slices() {
    let slices = ConfigSlices::default();
    assert_eq!(slices.version, SLICE_VERSION);
    assert!(!slices.dns.enabled);
    assert!(!slices.outbounds.enabled);
    assert!(slices.dns.servers.is_empty());
}

#[test]
fn missing_fields_fall_back_to_defaults() {
    let slices: ConfigSlices = serde_json::from_str("{}").unwrap();
    assert_eq!(slices, ConfigSlices::default());

    let dns: DnsSlice = serde_json::from_str("{}").unwrap();
    assert!(!dns.enabled);
    assert_eq!(dns.mode, DnsMode::FollowSystem);
    assert_eq!(dns.strategy, DnsStrategy::PreferIpv4);
    assert!(dns.final_tag.is_empty());
}

#[test]
fn outbound_missing_optional_fields_default() {
    let item: CustomOutbound = serde_json::from_str(
        r#"{
            "id": "o1",
            "name": "My Node",
            "type": "vless",
            "server": "example.com",
            "server_port": 443,
            "uuid": "uuid-1"
        }"#,
    )
    .unwrap();
    assert!(item.enabled);
    let OutboundProtocol::Vless(vless) = item.protocol else {
        panic!("expected vless protocol");
    };
    assert!(vless.flow.is_empty());
    assert!(!vless.tls.enabled);
    assert!(vless.transport.kind.is_empty());
}

#[test]
fn config_slices_roundtrip() {
    let slices = sample_slices();
    let text = serde_json::to_string(&slices).unwrap();
    let back: ConfigSlices = serde_json::from_str(&text).unwrap();
    assert_eq!(slices, back);
}

#[test]
fn validate_accepts_sample() {
    sample_slices().validate().unwrap();
}

#[test]
fn validate_rejects_duplicate_dns_server_tag() {
    let mut slices = sample_slices();
    slices.dns.servers[1].tag = "local".to_string();
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("duplicate DNS server tag"),
        "{err}"
    );
}

#[test]
fn validate_rejects_empty_and_whitespace_dns_tag() {
    let mut slices = sample_slices();
    slices.dns.servers[0].tag = String::new();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("must not be empty"), "{err}");

    let mut slices = sample_slices();
    slices.dns.servers[0].tag = "bad tag".to_string();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("whitespace"), "{err}");
}

#[test]
fn validate_rejects_dangling_final() {
    let mut slices = sample_slices();
    slices.dns.final_tag = "missing".to_string();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("does not reference"), "{err}");
}

#[test]
fn validate_rejects_takeover_without_final() {
    let mut slices = sample_slices();
    slices.dns.final_tag = String::new();
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("required when DNS takeover"),
        "{err}"
    );
}

#[test]
fn validate_rejects_rule_referencing_unknown_server() {
    let mut slices = sample_slices();
    slices.dns.rules[0].server_tag = "nope".to_string();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("unknown DNS server"), "{err}");
}

#[test]
fn validate_rejects_dns_server_port_zero() {
    let mut slices = sample_slices();
    slices.dns.servers[0].server_port = Some(0);
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("invalid port 0"), "{err}");
}

#[test]
fn validate_rejects_outbound_port_zero() {
    let mut slices = sample_slices();
    if let OutboundProtocol::Shadowsocks(ss) = &mut slices.outbounds.items[1].protocol {
        ss.server_port = 0;
    }
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("invalid port 0"), "{err}");
}

#[test]
fn validate_rejects_empty_outbound_server() {
    let mut slices = sample_slices();
    if let OutboundProtocol::Shadowsocks(ss) = &mut slices.outbounds.items[1].protocol {
        ss.server = String::new();
    }
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("requires a server address"),
        "{err}"
    );
}

#[test]
fn validate_rejects_duplicate_outbound_tags() {
    let mut slices = sample_slices();
    slices.outbounds.items[1].name = "My Vless".to_string();
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("duplicate custom outbound tag"),
        "{err}"
    );
}

#[test]
fn validate_skips_disabled_outbounds() {
    let mut slices = sample_slices();
    slices.outbounds.items[1].enabled = false;
    slices.outbounds.items[1].name = "My Vless".to_string();
    slices.outbounds.items[1].protocol = OutboundProtocol::Shadowsocks(ShadowsocksOutbound {
        server: String::new(),
        server_port: 0,
        method: String::new(),
        password: String::new(),
    });
    slices.validate().unwrap();
}

#[test]
fn outbound_tag_slugifies_name() {
    assert_eq!(outbound_tag("My Node 1"), "slice-my-node-1");
    assert_eq!(outbound_tag("  My__Node!! "), "slice-my-node");
    assert_eq!(outbound_tag("香港 01"), "slice-香港-01");
    assert_eq!(outbound_tag("   "), "slice-outbound");
    assert_eq!(outbound_tag("!!!"), "slice-outbound");
}

#[test]
fn outbound_tag_is_used_by_custom_outbound() {
    let item = CustomOutbound {
        id: "o1".to_string(),
        name: "My Node".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Shadowsocks(ShadowsocksOutbound::default()),
    };
    assert_eq!(item.tag(), "slice-my-node");
}

fn group_item(id: &str, name: &str, protocol: OutboundProtocol) -> CustomOutbound {
    CustomOutbound {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        protocol,
    }
}

fn selector(members: &[&str], default: &str) -> OutboundProtocol {
    OutboundProtocol::Selector(SelectorOutbound {
        outbounds: members.iter().map(ToString::to_string).collect(),
        default: default.to_string(),
        interrupt_exist_connections: false,
    })
}

fn urltest(members: &[&str]) -> OutboundProtocol {
    OutboundProtocol::UrlTest(UrlTestOutbound {
        outbounds: members.iter().map(ToString::to_string).collect(),
        ..Default::default()
    })
}

fn slices_with_group(group: CustomOutbound) -> ConfigSlices {
    let mut slices = sample_slices();
    slices.outbounds.items.push(group);
    slices
}

#[test]
fn group_outbounds_serde_roundtrip() {
    let selector = group_item(
        "g1",
        "Auto Select",
        selector(&["slice-my-vless", "direct"], "slice-my-vless"),
    );
    let text = serde_json::to_string(&selector).unwrap();
    assert!(text.contains("\"type\":\"selector\""), "{text}");
    let back: CustomOutbound = serde_json::from_str(&text).unwrap();
    assert_eq!(selector, back);

    let urltest = group_item(
        "g2",
        "Auto Test",
        OutboundProtocol::UrlTest(UrlTestOutbound {
            outbounds: vec!["slice-my-vless".to_string()],
            url: "https://example.com/ping".to_string(),
            interval: "5m".to_string(),
            tolerance: 80,
            interrupt_exist_connections: true,
        }),
    );
    let text = serde_json::to_string(&urltest).unwrap();
    // The sing-box type discriminator is `urltest`, not `url_test`.
    assert!(text.contains("\"type\":\"urltest\""), "{text}");
    let back: CustomOutbound = serde_json::from_str(&text).unwrap();
    assert_eq!(urltest, back);
}

#[test]
fn urltest_missing_fields_use_singbox_defaults() {
    let item: CustomOutbound = serde_json::from_str(
        r#"{"id":"g1","name":"Auto","type":"urltest","outbounds":["direct"]}"#,
    )
    .unwrap();
    let OutboundProtocol::UrlTest(o) = item.protocol else {
        panic!("expected urltest protocol");
    };
    assert_eq!(o.url, "https://www.gstatic.com/generate_204");
    assert_eq!(o.interval, "3m");
    assert_eq!(o.tolerance, 0);
    assert!(!o.interrupt_exist_connections);
}

#[test]
fn validate_accepts_group_referencing_nodes_direct_and_passthrough() {
    let slices = slices_with_group(group_item(
        "g1",
        "Auto",
        selector(
            &[
                "slice-my-vless",
                "slice-ss-node",
                "direct",
                "subscription-node",
            ],
            "slice-my-vless",
        ),
    ));
    slices.validate().unwrap();
}

#[test]
fn validate_accepts_urltest_group() {
    let slices = slices_with_group(group_item("g1", "Auto", urltest(&["slice-ss-node"])));
    slices.validate().unwrap();
}

#[test]
fn validate_rejects_empty_group_members() {
    let slices = slices_with_group(group_item("g1", "Auto", selector(&[], "")));
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("at least one member"), "{err}");
}

#[test]
fn validate_rejects_blank_group_member() {
    let slices = slices_with_group(group_item("g1", "Auto", urltest(&["  "])));
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("empty member tag"), "{err}");
}

#[test]
fn validate_rejects_nested_group() {
    let mut slices = sample_slices();
    slices.outbounds.items.push(group_item(
        "g1",
        "Group One",
        selector(&["slice-my-vless"], ""),
    ));
    slices.outbounds.items.push(group_item(
        "g2",
        "Group Two",
        selector(&["slice-group-one"], ""),
    ));
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("another group"), "{err}");
}

#[test]
fn validate_rejects_self_reference() {
    let slices = slices_with_group(group_item("g1", "Loop", selector(&["slice-loop"], "")));
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("must not reference itself"),
        "{err}"
    );
}

#[test]
fn validate_rejects_selector_default_not_member() {
    let slices = slices_with_group(group_item(
        "g1",
        "Auto",
        selector(&["direct"], "slice-my-vless"),
    ));
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("is not one of its members"),
        "{err}"
    );
}

#[test]
fn validate_rejects_unknown_slice_member() {
    let slices = slices_with_group(group_item("g1", "Auto", urltest(&["slice-missing"])));
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string()
            .contains("references unknown custom outbound"),
        "{err}"
    );
}

#[test]
fn validate_rejects_group_member_when_referenced_node_disabled() {
    let mut slices = sample_slices();
    slices.outbounds.items[1].enabled = false; // disable "ss node"
    slices
        .outbounds
        .items
        .push(group_item("g1", "Auto", urltest(&["slice-ss-node"])));
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string()
            .contains("references unknown custom outbound"),
        "{err}"
    );
}
