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
