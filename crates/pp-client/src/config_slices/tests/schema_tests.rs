//! Schema + validation tests for config slices.
//!
//! Split out of `schema.rs` to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use crate::config_slices::*;

fn dns_slice() -> DnsSlice {
    DnsSlice {
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
            action: DnsRuleAction::Route,
            rcode: String::new(),
        }],
        final_tag: "remote".to_string(),
        strategy: DnsStrategy::PreferIpv4,
        reverse_mapping: false,
    }
}

fn outbounds_slice() -> OutboundsSlice {
    OutboundsSlice {
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
        experimental: ExperimentalSlice::default(),
        route: RouteSlice::default(),
    }
}

#[test]
fn default_has_version_and_empty_slices() {
    let slices = ConfigSlices::default();
    assert_eq!(slices.version, SLICE_VERSION);
    assert_eq!(slices.dns.mode, DnsMode::FollowSystem);
    assert!(slices.dns.servers.is_empty());
    assert!(slices.outbounds.items.is_empty());
    assert!(!slices.experimental.cache_file.enabled);
}

#[test]
fn missing_fields_fall_back_to_defaults() {
    let slices: ConfigSlices = serde_json::from_str("{}").unwrap();
    assert_eq!(slices, ConfigSlices::default());

    let dns: DnsSlice = serde_json::from_str("{}").unwrap();
    assert_eq!(dns.mode, DnsMode::FollowSystem);
    assert_eq!(dns.strategy, DnsStrategy::PreferIpv4);
    assert!(dns.final_tag.is_empty());
}

/// v1 documents still parse: the removed per-slice `enabled` keys are ignored.
#[test]
fn legacy_enabled_fields_are_ignored_by_serde() {
    let slices: ConfigSlices = serde_json::from_str(
        r#"{
            "version": 1,
            "dns": { "enabled": true, "mode": "takeover", "final_tag": "remote" },
            "outbounds": { "enabled": true, "items": [] },
            "experimental": { "enabled": true, "cache_file": { "enabled": true } },
            "route": { "enabled": true, "final_tag": "direct" }
        }"#,
    )
    .unwrap();
    assert_eq!(slices.version, 1);
    assert_eq!(slices.dns.mode, DnsMode::Takeover);
    assert_eq!(slices.route.final_tag, "direct");
    assert!(slices.experimental.cache_file.enabled);
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

/// 内置 `block` 与 `direct` 一样可显式作分组成员（创建分组时可选内置出站）。
#[test]
fn validate_accepts_group_referencing_builtin_block() {
    let slices = slices_with_group(group_item(
        "g1",
        "Auto",
        selector(&["slice-my-vless", "direct", "block"], "slice-my-vless"),
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

#[test]
fn dns_rule_and_server_old_data_default_new_fields() {
    // Legacy DNS rule without `action` / `rcode` stays a route rule.
    let rule: DnsRule = serde_json::from_str(
        r#"{"id":"r1","match_type":"domain_suffix","target":".cn","server_tag":"local"}"#,
    )
    .unwrap();
    assert_eq!(rule.action, DnsRuleAction::Route);
    assert!(rule.rcode.is_empty());

    // Legacy DNS server without FakeIP ranges defaults them to empty.
    let server: DnsServer =
        serde_json::from_str(r#"{"tag":"s","server":"1.1.1.1","server_type":"udp"}"#).unwrap();
    assert!(server.inet4_range.is_empty());
    assert!(server.inet6_range.is_empty());

    let fakeip: DnsServer = serde_json::from_str(r#"{"tag":"f","server_type":"fakeip"}"#).unwrap();
    assert_eq!(fakeip.server_type, DnsServerType::Fakeip);
}

#[test]
fn validate_accepts_fakeip_server() {
    let mut slices = sample_slices();
    slices.dns.servers.push(DnsServer {
        tag: "fake".to_string(),
        server_type: DnsServerType::Fakeip,
        inet4_range: "198.18.0.0/15".to_string(),
        inet6_range: "fc00::/18".to_string(),
        // Server fields are ignored for fakeip.
        server: "1.1.1.1".to_string(),
        ..Default::default()
    });
    slices.validate().unwrap();
}

#[test]
fn validate_rejects_fakeip_bad_cidr() {
    let mut slices = sample_slices();
    slices.dns.servers.push(DnsServer {
        tag: "fake".to_string(),
        server_type: DnsServerType::Fakeip,
        inet4_range: "not-a-cidr".to_string(),
        ..Default::default()
    });
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("is not a CIDR range"), "{err}");

    let mut slices = sample_slices();
    slices.dns.servers.push(DnsServer {
        tag: "fake".to_string(),
        server_type: DnsServerType::Fakeip,
        inet6_range: "fc00::/".to_string(),
        ..Default::default()
    });
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("inet6_range"), "{err}");
}

#[test]
fn validate_accepts_query_type_rule() {
    let mut slices = sample_slices();
    slices.dns.rules[0].match_type = DnsMatchType::QueryType;
    slices.dns.rules[0].target = "a, AAAA".to_string();
    slices.validate().unwrap();
}

#[test]
fn validate_rejects_invalid_query_type() {
    let mut slices = sample_slices();
    slices.dns.rules[0].match_type = DnsMatchType::QueryType;
    slices.dns.rules[0].target = "A,NOTATYPE".to_string();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("invalid query type"), "{err}");
}

#[test]
fn validate_rejects_empty_query_type_target() {
    let mut slices = sample_slices();
    slices.dns.rules[0].match_type = DnsMatchType::QueryType;
    slices.dns.rules[0].target = "  ".to_string();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("must not be empty"), "{err}");
}

#[test]
fn validate_accepts_predefined_and_reject_actions() {
    let mut slices = sample_slices();
    slices.dns.rules = vec![
        DnsRule {
            id: "predef".to_string(),
            enabled: true,
            match_type: DnsMatchType::Domain,
            target: "ads.example".to_string(),
            server_tag: String::new(),
            action: DnsRuleAction::Predefined,
            rcode: "nxdomain".to_string(),
        },
        DnsRule {
            id: "reject".to_string(),
            enabled: true,
            match_type: DnsMatchType::DomainKeyword,
            target: "tracker".to_string(),
            server_tag: String::new(),
            action: DnsRuleAction::Reject,
            rcode: String::new(),
        },
    ];
    slices.validate().unwrap();
}

#[test]
fn validate_rejects_predefined_with_server_tag() {
    let mut slices = sample_slices();
    slices.dns.rules[0].action = DnsRuleAction::Predefined;
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("must not set a server tag"),
        "{err}"
    );
}

#[test]
fn validate_rejects_reject_with_server_tag() {
    let mut slices = sample_slices();
    slices.dns.rules[0].action = DnsRuleAction::Reject;
    let err = slices.validate().unwrap_err();
    assert!(
        err.to_string().contains("must not set a server tag"),
        "{err}"
    );
}

#[test]
fn validate_rejects_invalid_predefined_rcode() {
    let mut slices = sample_slices();
    slices.dns.rules[0].action = DnsRuleAction::Predefined;
    slices.dns.rules[0].server_tag = String::new();
    slices.dns.rules[0].rcode = "BOGUS".to_string();
    let err = slices.validate().unwrap_err();
    assert!(err.to_string().contains("invalid rcode"), "{err}");
}

/// A `clash_mode` DNS rule with a target outside `rule`/`global`/`direct` is rejected.
#[test]
fn dns_slice_rejects_invalid_clash_mode_target() {
    let mut slice = dns_slice();
    slice.rules = vec![DnsRule {
        id: "r-mode".to_string(),
        enabled: true,
        match_type: DnsMatchType::ClashMode,
        target: "turbo".to_string(),
        server_tag: "local".to_string(),
        action: DnsRuleAction::Route,
        rcode: String::new(),
    }];
    assert!(slice.validate().is_err());

    slice.rules[0].target = "direct".to_string();
    slice.validate().unwrap();
}

/// `dns.final` / DNS 规则引用弃用（disabled）服务器是校验错误（引用会在运行配置中悬空）。
#[test]
fn dns_slice_rejects_references_to_disabled_servers() {
    let mut slice = dns_slice();
    // final 指向弃用的 remote。
    slice.servers[1].enabled = false;
    let err = slice.validate().unwrap_err();
    assert!(err.to_string().contains("disabled"), "{err}");

    // 恢复 final 目标启用，让规则指向弃用服务器。
    let mut slice = dns_slice();
    slice.servers[0].enabled = false; // local 弃用
    slice.rules = vec![DnsRule {
        id: "r-disabled".to_string(),
        enabled: true,
        match_type: DnsMatchType::Domain,
        target: "example.com".to_string(),
        server_tag: "local".to_string(),
        action: DnsRuleAction::Route,
        rcode: String::new(),
    }];
    slice.final_tag = "remote".to_string();
    let err = slice.validate().unwrap_err();
    assert!(err.to_string().contains("disabled"), "{err}");

    // 弃用服务器的 tag 仍占用命名空间（重复 tag 仍冲突）。
    let mut slice = dns_slice();
    slice.servers[0].enabled = false;
    slice.servers.push(DnsServer {
        tag: "local".to_string(),
        server: "9.9.9.9".to_string(),
        server_type: DnsServerType::Udp,
        ..Default::default()
    });
    assert!(
        slice.validate().is_err(),
        "duplicate tag with disabled server"
    );
}

/// rule_set 目标全为空白段（如 `" , ,"`）时校验报错；多 tag 混合空白段合法。
#[test]
fn dns_slice_rule_set_target_requires_one_non_empty_tag() {
    let mut slice = dns_slice();
    slice.rules = vec![DnsRule {
        id: "r-rs".to_string(),
        enabled: true,
        match_type: DnsMatchType::RuleSet,
        target: " , ,".to_string(),
        server_tag: "local".to_string(),
        action: DnsRuleAction::Route,
        rcode: String::new(),
    }];
    assert!(slice.validate().is_err());

    slice.rules[0].target = "geosite-cn, , geosite-private".to_string();
    slice.validate().unwrap();
}
