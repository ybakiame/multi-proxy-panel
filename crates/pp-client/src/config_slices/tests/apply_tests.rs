//! Pure render + apply tests for config slices.
//!
//! Split out of `apply.rs` to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use crate::config_slices::*;
use serde_json::json;

fn dns_slice() -> DnsSlice {
    DnsSlice {
        mode: DnsMode::Takeover,
        servers: vec![
            DnsServer {
                tag: "local".to_string(),
                server_type: DnsServerType::Local,
                ..Default::default()
            },
            DnsServer {
                tag: "remote".to_string(),
                server: "1.1.1.1".to_string(),
                server_type: DnsServerType::Https,
                server_port: Some(443),
                detour: "proxy".to_string(),
                domain_resolver: "local".to_string(),
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
        strategy: DnsStrategy::PreferIpv6,
        reverse_mapping: false,
    }
}

fn ss_outbound(id: &str, name: &str) -> CustomOutbound {
    CustomOutbound {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        protocol: OutboundProtocol::Shadowsocks(ShadowsocksOutbound {
            server: "1.2.3.4".to_string(),
            server_port: 8388,
            method: "aes-256-gcm".to_string(),
            password: "secret".to_string(),
        }),
        builtin: false,
    }
}

#[test]
fn render_dns_uses_type_based_new_format() {
    let value = render_dns(&dns_slice());
    assert_eq!(value["servers"][0]["tag"], "local");
    assert_eq!(value["servers"][0]["type"], "local");
    assert!(value["servers"][0].get("server").is_none());
    assert!(value["servers"][0].get("server_port").is_none());

    assert_eq!(value["servers"][1]["type"], "https");
    assert_eq!(value["servers"][1]["server"], "1.1.1.1");
    assert_eq!(value["servers"][1]["server_port"], 443);
    assert_eq!(value["servers"][1]["detour"], "proxy");
    assert_eq!(value["servers"][1]["domain_resolver"], "local");

    assert_eq!(value["rules"][0]["domain_suffix"], json!([".cn"]));
    assert_eq!(value["rules"][0]["action"], "route");
    assert_eq!(value["rules"][0]["server"], "local");
    assert_eq!(value["final"], "remote");
    assert_eq!(value["strategy"], "prefer_ipv6");
}

#[test]
fn render_dns_skips_disabled_rules() {
    let mut dns = dns_slice();
    dns.rules[0].enabled = false;
    let value = render_dns(&dns);
    assert!(value["rules"].as_array().unwrap().is_empty());
}

#[test]
fn apply_replaces_dns_object() {
    let mut config = json!({
        "dns": { "servers": [ { "tag": "old", "type": "udp", "server": "9.9.9.9" } ] },
        "outbounds": []
    });
    let slices = ConfigSlices {
        dns: dns_slice(),
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert!(report.dns_applied);
    assert_eq!(config["dns"]["servers"][0]["tag"], "local");
    assert!(
        config["dns"]["servers"]
            .as_array()
            .unwrap()
            .iter()
            .all(|server| server["tag"] != "old")
    );
    assert_eq!(config["dns"]["strategy"], "prefer_ipv6");
}

#[test]
fn apply_appends_outbounds() {
    let mut config = json!({ "outbounds": [ { "type": "direct", "tag": "direct" } ] });
    let slices = ConfigSlices {
        outbounds: OutboundsSlice {
            items: vec![ss_outbound("o1", "Node One")],
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert_eq!(report.outbound_tags, vec!["slice-node-one".to_string()]);
    assert!(report.renamed_outbounds.is_empty());

    let outbounds = config["outbounds"].as_array().unwrap();
    assert_eq!(outbounds.len(), 2);
    assert_eq!(outbounds[0]["tag"], "direct");
    assert_eq!(outbounds[1]["tag"], "slice-node-one");
    assert_eq!(outbounds[1]["type"], "shadowsocks");
    assert_eq!(outbounds[1]["method"], "aes-256-gcm");
}

#[test]
fn apply_renames_conflicting_tag_with_stable_suffix() {
    let mut config = json!({
        "outbounds": [
            { "type": "direct", "tag": "slice-node-one" },
            { "type": "block", "tag": "slice-node-one-2" }
        ]
    });
    let slices = ConfigSlices {
        outbounds: OutboundsSlice {
            items: vec![ss_outbound("o1", "Node One")],
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert_eq!(report.outbound_tags, vec!["slice-node-one-3".to_string()]);
    assert_eq!(
        report.renamed_outbounds,
        vec![OutboundTagRename {
            from: "slice-node-one".to_string(),
            to: "slice-node-one-3".to_string(),
        }]
    );
    assert_eq!(config["outbounds"][2]["tag"], "slice-node-one-3");
}

#[test]
fn apply_skips_disabled_outbound_items() {
    let mut config = json!({ "outbounds": [] });
    let mut item = ss_outbound("o1", "Node One");
    item.enabled = false;
    let slices = ConfigSlices {
        outbounds: OutboundsSlice { items: vec![item] },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert!(report.outbound_tags.is_empty());
    assert!(config["outbounds"].as_array().unwrap().is_empty());
}

#[test]
fn apply_disabled_slices_leaves_config_unchanged() {
    let mut config = json!({
        "dns": { "servers": [] },
        "outbounds": [ { "type": "direct", "tag": "direct" } ]
    });
    let original = config.clone();

    let report = apply_config_slices(&mut config, &ConfigSlices::default()).unwrap();
    assert_eq!(config, original);
    assert_eq!(report, ApplyReport::default());
}

#[test]
fn apply_errors_on_non_object_config() {
    let mut config = json!([1, 2, 3]);
    let err = apply_config_slices(&mut config, &ConfigSlices::default()).unwrap_err();
    assert!(err.to_string().contains("not a JSON object"), "{err}");
}

/// Content-driven injection: DNS (takeover) + outbounds + experimental are all
/// applied without any master switch.
#[test]
fn apply_injects_content_driven_slices() {
    let mut config = json!({
        "dns": { "servers": [] },
        "outbounds": [ { "type": "direct", "tag": "direct" } ]
    });
    let slices = ConfigSlices {
        dns: dns_slice(),
        outbounds: OutboundsSlice {
            items: vec![ss_outbound("o1", "Node One")],
        },
        experimental: ExperimentalSlice {
            cache_file: CacheFileSlice {
                enabled: true,
                path: "/data/cache.db".to_string(),
                ..Default::default()
            },
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert!(report.dns_applied);
    assert_eq!(report.outbound_tags, vec!["slice-node-one".to_string()]);
    assert_eq!(config["outbounds"].as_array().unwrap().len(), 2);
    assert_eq!(config["experimental"]["cache_file"]["enabled"], true);
    assert_eq!(
        config["experimental"]["cache_file"]["path"],
        "/data/cache.db"
    );
}

/// `FollowSystem` DNS mode does not inject the slice body (desktop and Android
/// share this semantics); the built-in/template DNS is preserved.
#[test]
fn apply_follow_system_dns_does_not_inject() {
    let mut config = json!({
        "dns": { "servers": [{ "tag": "builtin", "type": "udp", "server": "1.1.1.1" }] }
    });
    let original = config.clone();
    let slices = ConfigSlices {
        dns: DnsSlice {
            mode: DnsMode::FollowSystem,
            servers: vec![DnsServer {
                tag: "slice".to_string(),
                server: "9.9.9.9".to_string(),
                server_type: DnsServerType::Udp,
                ..Default::default()
            }],
            final_tag: "slice".to_string(),
            ..Default::default()
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert!(!report.dns_applied);
    assert_eq!(config, original);
}

/// Experimental cache_file disabled: no `experimental.cache_file` is written,
/// sibling keys survive untouched.
#[test]
fn apply_disabled_cache_file_leaves_experimental_untouched() {
    let mut config = json!({
        "experimental": { "clash_api": { "external_controller": "127.0.0.1:9090" } }
    });
    let original = config.clone();
    let slices = ConfigSlices {
        experimental: ExperimentalSlice {
            cache_file: CacheFileSlice {
                enabled: false,
                path: "/data/cache.db".to_string(),
                ..Default::default()
            },
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert_eq!(config, original);
    assert!(report.outbound_tags.is_empty());
}

#[test]
fn render_outbound_vless_with_tls_and_ws() {
    let item = CustomOutbound {
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
                server_name: "sni.example.com".to_string(),
                insecure: true,
                alpn: vec!["h2".to_string()],
            },
            transport: OutboundTransport {
                kind: "ws".to_string(),
                path: "/ws".to_string(),
                host: "host.example.com".to_string(),
            },
        }),
        builtin: false,
    };

    let value = render_outbound(&item, "slice-my-vless");
    assert_eq!(value["tag"], "slice-my-vless");
    assert_eq!(value["type"], "vless");
    assert_eq!(value["server"], "example.com");
    assert_eq!(value["server_port"], 443);
    assert_eq!(value["uuid"], "uuid-1");
    assert_eq!(value["flow"], "xtls-rprx-vision");
    assert_eq!(value["tls"]["enabled"], true);
    assert_eq!(value["tls"]["server_name"], "sni.example.com");
    assert_eq!(value["tls"]["insecure"], true);
    assert_eq!(value["tls"]["alpn"], json!(["h2"]));
    assert_eq!(value["transport"]["type"], "ws");
    assert_eq!(value["transport"]["path"], "/ws");
    assert_eq!(value["transport"]["headers"]["Host"], "host.example.com");
}

#[test]
fn render_outbound_protocol_specific_fields() {
    let vmess = CustomOutbound {
        id: "o1".to_string(),
        name: "vm".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Vmess(VmessOutbound {
            server: "a".to_string(),
            server_port: 80,
            uuid: "u".to_string(),
            security: "auto".to_string(),
            alter_id: 1,
            ..Default::default()
        }),
        builtin: false,
    };
    let value = render_outbound(&vmess, "slice-vm");
    assert_eq!(value["type"], "vmess");
    assert_eq!(value["security"], "auto");
    assert_eq!(value["alter_id"], 1);
    assert!(value.get("tls").is_none());
    assert!(value.get("transport").is_none());

    let trojan = CustomOutbound {
        id: "o2".to_string(),
        name: "tj".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Trojan(TrojanOutbound {
            server: "b".to_string(),
            server_port: 443,
            password: "pw".to_string(),
            tls: OutboundTls {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        }),
        builtin: false,
    };
    let value = render_outbound(&trojan, "slice-tj");
    assert_eq!(value["type"], "trojan");
    assert_eq!(value["password"], "pw");
    assert_eq!(value["tls"]["enabled"], true);

    let hy2 = CustomOutbound {
        id: "o3".to_string(),
        name: "hy".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Hysteria2(Hysteria2Outbound {
            server: "c".to_string(),
            server_port: 443,
            password: "pw".to_string(),
            up_mbps: 100,
            down_mbps: 200,
            obfs: Hysteria2Obfs {
                obfs_type: "salamander".to_string(),
                password: "obfs-pw".to_string(),
            },
            tls: OutboundTls {
                enabled: true,
                ..Default::default()
            },
        }),
        builtin: false,
    };
    let value = render_outbound(&hy2, "slice-hy");
    assert_eq!(value["type"], "hysteria2");
    assert_eq!(value["up_mbps"], 100);
    assert_eq!(value["down_mbps"], 200);
    assert_eq!(value["obfs"]["type"], "salamander");
    assert_eq!(value["obfs"]["password"], "obfs-pw");
}

#[test]
fn render_outbound_grpc_and_http_transports() {
    let grpc = OutboundTransport {
        kind: "grpc".to_string(),
        path: "TunService".to_string(),
        host: String::new(),
    };
    let item = CustomOutbound {
        id: "o1".to_string(),
        name: "g".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Vless(VlessOutbound {
            server: "a".to_string(),
            server_port: 443,
            uuid: "u".to_string(),
            transport: grpc,
            ..Default::default()
        }),
        builtin: false,
    };
    let value = render_outbound(&item, "slice-g");
    assert_eq!(value["transport"]["type"], "grpc");
    assert_eq!(value["transport"]["service_name"], "TunService");

    let http = OutboundTransport {
        kind: "http".to_string(),
        path: "/path".to_string(),
        host: "h.example.com".to_string(),
    };
    let item = CustomOutbound {
        id: "o2".to_string(),
        name: "h".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Vless(VlessOutbound {
            server: "a".to_string(),
            server_port: 443,
            uuid: "u".to_string(),
            transport: http,
            ..Default::default()
        }),
        builtin: false,
    };
    let value = render_outbound(&item, "slice-h");
    assert_eq!(value["transport"]["type"], "http");
    assert_eq!(value["transport"]["host"], json!(["h.example.com"]));
}

#[test]
fn render_outbound_selector_fields() {
    let item = CustomOutbound {
        id: "g1".to_string(),
        name: "Auto Select".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Selector(SelectorOutbound {
            outbounds: vec!["slice-a".to_string(), "direct".to_string()],
            default: "direct".to_string(),
            interrupt_exist_connections: true,
        }),
        builtin: false,
    };

    let value = render_outbound(&item, "slice-auto-select");
    assert_eq!(value["type"], "selector");
    assert_eq!(value["outbounds"], json!(["slice-a", "direct"]));
    assert_eq!(value["default"], "direct");
    assert_eq!(value["interrupt_exist_connections"], true);
}

#[test]
fn render_outbound_urltest_fields() {
    let item = CustomOutbound {
        id: "g1".to_string(),
        name: "Auto Test".to_string(),
        enabled: true,
        protocol: OutboundProtocol::UrlTest(UrlTestOutbound {
            outbounds: vec!["slice-a".to_string()],
            url: "https://example.com/ping".to_string(),
            interval: "5m".to_string(),
            tolerance: 80,
            interrupt_exist_connections: false,
        }),
        builtin: false,
    };

    let value = render_outbound(&item, "slice-auto-test");
    assert_eq!(value["type"], "urltest");
    assert_eq!(value["outbounds"], json!(["slice-a"]));
    assert_eq!(value["url"], "https://example.com/ping");
    assert_eq!(value["interval"], "5m");
    assert_eq!(value["tolerance"], 80);
    assert!(value.get("interrupt_exist_connections").is_none());
}

#[test]
fn render_urltest_omits_zero_tolerance_and_empty_optionals() {
    let item = CustomOutbound {
        id: "g1".to_string(),
        name: "Auto Test".to_string(),
        enabled: true,
        protocol: OutboundProtocol::UrlTest(UrlTestOutbound {
            outbounds: vec!["direct".to_string()],
            url: String::new(),
            interval: String::new(),
            tolerance: 0,
            interrupt_exist_connections: false,
        }),
        builtin: false,
    };

    let value = render_outbound(&item, "slice-auto-test");
    assert!(value.get("url").is_none());
    assert!(value.get("interval").is_none());
    assert!(value.get("tolerance").is_none());
    assert!(value.get("interrupt_exist_connections").is_none());
}

#[test]
fn render_selector_omits_empty_default() {
    let item = CustomOutbound {
        id: "g1".to_string(),
        name: "Auto Select".to_string(),
        enabled: true,
        protocol: OutboundProtocol::Selector(SelectorOutbound {
            outbounds: vec!["direct".to_string()],
            default: String::new(),
            interrupt_exist_connections: false,
        }),
        builtin: false,
    };

    let value = render_outbound(&item, "slice-auto-select");
    assert!(value.get("default").is_none());
    assert!(value.get("interrupt_exist_connections").is_none());
}

#[test]
fn apply_renders_groups_after_nodes_and_remaps_members() {
    let mut config = json!({ "outbounds": [ { "type": "direct", "tag": "slice-node" } ] });
    let slices = ConfigSlices {
        outbounds: OutboundsSlice {
            items: vec![
                ss_outbound("o1", "Node"),
                CustomOutbound {
                    id: "g1".to_string(),
                    name: "Auto".to_string(),
                    enabled: true,
                    protocol: OutboundProtocol::Selector(SelectorOutbound {
                        outbounds: vec!["slice-node".to_string(), "direct".to_string()],
                        default: "slice-node".to_string(),
                        interrupt_exist_connections: false,
                    }),
                    builtin: false,
                },
            ],
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    // Node is renamed (existing `slice-node` tag), group keeps its own tag.
    assert_eq!(
        report.outbound_tags,
        vec!["slice-node-2".to_string(), "slice-auto".to_string()]
    );

    let outbounds = config["outbounds"].as_array().unwrap();
    assert_eq!(outbounds.len(), 3);
    assert_eq!(outbounds[0]["tag"], "slice-node");
    assert_eq!(outbounds[1]["tag"], "slice-node-2");
    assert_eq!(outbounds[2]["tag"], "slice-auto");
    assert_eq!(outbounds[2]["type"], "selector");
    // The group is rendered after its member and the renamed tag is remapped.
    assert_eq!(outbounds[2]["outbounds"], json!(["slice-node-2", "direct"]));
    assert_eq!(outbounds[2]["default"], "slice-node-2");
}

#[test]
fn apply_skips_disabled_group_outbounds() {
    let mut config = json!({ "outbounds": [ { "type": "direct", "tag": "direct" } ] });
    let group = CustomOutbound {
        id: "g1".to_string(),
        name: "Auto".to_string(),
        enabled: false,
        protocol: OutboundProtocol::UrlTest(UrlTestOutbound {
            outbounds: vec!["direct".to_string()],
            ..Default::default()
        }),
        builtin: false,
    };
    let slices = ConfigSlices {
        outbounds: OutboundsSlice { items: vec![group] },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert!(report.outbound_tags.is_empty());
    assert_eq!(config["outbounds"].as_array().unwrap().len(), 1);
}

#[test]
fn render_fakeip_server_uses_ranges_and_omits_dial_fields() {
    let dns = DnsSlice {
        mode: DnsMode::Takeover,
        servers: vec![
            DnsServer {
                tag: "fake".to_string(),
                server_type: DnsServerType::Fakeip,
                ..Default::default()
            },
            DnsServer {
                tag: "fake-v6".to_string(),
                server_type: DnsServerType::Fakeip,
                inet4_range: "10.0.0.0/8".to_string(),
                inet6_range: "fc00::/18".to_string(),
                // Must be ignored for fakeip servers.
                server: "1.1.1.1".to_string(),
                server_port: Some(53),
                detour: "proxy".to_string(),
                ..Default::default()
            },
        ],
        final_tag: "fake".to_string(),
        ..Default::default()
    };

    let value = render_dns(&dns);
    assert_eq!(value["servers"][0]["type"], "fakeip");
    assert_eq!(value["servers"][0]["inet4_range"], "198.18.0.0/15");
    assert!(value["servers"][0].get("inet6_range").is_none());
    assert!(value["servers"][0].get("server").is_none());
    assert!(value["servers"][0].get("server_port").is_none());
    assert!(value["servers"][0].get("detour").is_none());

    assert_eq!(value["servers"][1]["inet4_range"], "10.0.0.0/8");
    assert_eq!(value["servers"][1]["inet6_range"], "fc00::/18");
    assert!(value["servers"][1].get("server").is_none());
    assert!(value["servers"][1].get("server_port").is_none());
    assert!(value["servers"][1].get("detour").is_none());
}

#[test]
fn render_non_fakeip_server_omits_inet_ranges() {
    let dns = DnsSlice {
        servers: vec![DnsServer {
            tag: "udp".to_string(),
            server: "1.1.1.1".to_string(),
            server_type: DnsServerType::Udp,
            server_port: Some(53),
            inet4_range: "198.18.0.0/15".to_string(),
            inet6_range: "fc00::/18".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };

    let value = render_dns(&dns);
    assert_eq!(value["servers"][0]["type"], "udp");
    assert_eq!(value["servers"][0]["server"], "1.1.1.1");
    assert!(value["servers"][0].get("inet4_range").is_none());
    assert!(value["servers"][0].get("inet6_range").is_none());
}

#[test]
fn render_query_type_rule_as_uppercased_array() {
    let mut dns = dns_slice();
    dns.rules = vec![DnsRule {
        id: "r-qt".to_string(),
        enabled: true,
        match_type: DnsMatchType::QueryType,
        target: "a, Aaaa ,https".to_string(),
        server_tag: "local".to_string(),
        action: DnsRuleAction::Route,
        rcode: String::new(),
    }];

    let value = render_dns(&dns);
    assert_eq!(
        value["rules"][0]["query_type"],
        json!(["A", "AAAA", "HTTPS"])
    );
    assert_eq!(value["rules"][0]["action"], "route");
    assert_eq!(value["rules"][0]["server"], "local");
}

#[test]
fn render_query_type_single_value_is_still_array() {
    let mut dns = dns_slice();
    dns.rules[0].match_type = DnsMatchType::QueryType;
    dns.rules[0].target = "A".to_string();

    let value = render_dns(&dns);
    assert_eq!(value["rules"][0]["query_type"], json!(["A"]));
}

#[test]
fn render_dns_rule_actions() {
    let mut dns = dns_slice();
    dns.rules = vec![
        DnsRule {
            id: "route".to_string(),
            enabled: true,
            match_type: DnsMatchType::DomainSuffix,
            target: ".cn".to_string(),
            server_tag: "local".to_string(),
            action: DnsRuleAction::Route,
            rcode: String::new(),
        },
        DnsRule {
            id: "predef".to_string(),
            enabled: true,
            match_type: DnsMatchType::Domain,
            target: "ads.example".to_string(),
            server_tag: String::new(),
            action: DnsRuleAction::Predefined,
            rcode: String::new(),
        },
        DnsRule {
            id: "predef-nx".to_string(),
            enabled: true,
            match_type: DnsMatchType::Domain,
            target: "bad.example".to_string(),
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

    let value = render_dns(&dns);
    let rules = value["rules"].as_array().unwrap();

    assert_eq!(rules[0]["action"], "route");
    assert_eq!(rules[0]["server"], "local");
    assert!(rules[0].get("rcode").is_none());

    assert_eq!(rules[1]["action"], "predefined");
    assert_eq!(rules[1]["rcode"], "NOERROR");
    assert!(rules[1].get("server").is_none());

    assert_eq!(rules[2]["action"], "predefined");
    assert_eq!(rules[2]["rcode"], "NXDOMAIN");

    assert_eq!(rules[3]["action"], "reject");
    assert!(rules[3].get("server").is_none());
    assert!(rules[3].get("rcode").is_none());
}

/// 弃用（`enabled = false`）的服务器保留在切片中但不渲染进 `dns.servers`；
/// `Default::default()` 构造的服务器默认 `enabled = true`（不静默产出弃用项）。
#[test]
fn render_dns_skips_disabled_servers() {
    let mut slice = dns_slice();
    slice.servers[0].enabled = false; // local 弃用，只剩 remote
    let rendered = render_dns(&slice);
    let servers = rendered["servers"].as_array().unwrap();
    assert_eq!(servers.len(), 1, "disabled server must not render");
    assert_eq!(servers[0]["tag"], "remote");

    let default_server = DnsServer::default();
    assert!(
        default_server.enabled,
        "Default must produce enabled server"
    );
}

/// rule_set 目标为逗号分隔多 tag 时渲染为 sing-box 数组（单值保持单元素数组）。
#[test]
fn render_dns_rule_set_target_multi_tag_array() {
    let mut slice = dns_slice();
    slice.rules = vec![
        DnsRule {
            id: "r-multi".to_string(),
            enabled: true,
            match_type: DnsMatchType::RuleSet,
            target: "geosite-cn, geosite-private,,".to_string(),
            server_tag: "local".to_string(),
            action: DnsRuleAction::Route,
            rcode: String::new(),
        },
        DnsRule {
            id: "r-single".to_string(),
            enabled: true,
            match_type: DnsMatchType::RuleSet,
            target: "geosite-cn".to_string(),
            server_tag: "local".to_string(),
            action: DnsRuleAction::Route,
            rcode: String::new(),
        },
    ];
    let rendered = render_dns(&slice);
    assert_eq!(
        rendered["rules"][0]["rule_set"],
        serde_json::json!(["geosite-cn", "geosite-private"]),
        "comma-separated target renders as a trimmed non-empty array"
    );
    assert_eq!(
        rendered["rules"][1]["rule_set"],
        serde_json::json!(["geosite-cn"]),
        "single value stays a single-element array"
    );
}

/// 内置分组条目不追加新出站，而是把可调字段覆写到模板同名分组（2026-09 物化模型）：
/// - selector：`default` 属于当前成员列表才覆写（悬空引用保持模板默认并告警）；
/// - urltest：`url` / `interval` / `tolerance` / interrupt 覆写；成员列表保持模板动态计算。
#[test]
fn apply_builtin_group_entries_override_template_groups() {
    let mut cfg = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["auto", "n1"], "default": "auto" },
            { "type": "urltest", "tag": "auto", "outbounds": ["n1"], "url": "https://www.gstatic.com/generate_204", "interval": "5m", "tolerance": 150 }
        ]
    });
    let mut slices = ConfigSlices::default();
    slices.outbounds.items = vec![
        CustomOutbound {
            id: "builtin-group-proxy".to_string(),
            name: "proxy".to_string(),
            enabled: true,
            builtin: true,
            protocol: OutboundProtocol::Selector(SelectorOutbound {
                outbounds: vec![],
                default: "n1".to_string(),
                interrupt_exist_connections: true,
            }),
        },
        CustomOutbound {
            id: "builtin-group-auto".to_string(),
            name: "auto".to_string(),
            enabled: true,
            builtin: true,
            protocol: OutboundProtocol::UrlTest(UrlTestOutbound {
                outbounds: vec![],
                url: "https://cp.cloudflare.com/".to_string(),
                interval: "3m".to_string(),
                tolerance: 300,
                interrupt_exist_connections: true,
            }),
        },
    ];
    apply_config_slices(&mut cfg, &slices).unwrap();

    assert_eq!(
        cfg["outbounds"].as_array().unwrap().len(),
        2,
        "builtin entries must not append new outbounds"
    );
    let proxy = &cfg["outbounds"][0];
    assert_eq!(proxy["default"], "n1", "selector default overridden");
    assert_eq!(proxy["interrupt_exist_connections"], true);
    assert_eq!(
        proxy["outbounds"],
        json!(["auto", "n1"]),
        "members stay template-computed"
    );
    let auto = &cfg["outbounds"][1];
    assert_eq!(auto["url"], "https://cp.cloudflare.com/");
    assert_eq!(auto["interval"], "3m");
    assert_eq!(auto["tolerance"], 300);
    assert_eq!(auto["interrupt_exist_connections"], true);

    // 悬空 default（成员里没有）不覆写。
    slices.outbounds.items[0].protocol = OutboundProtocol::Selector(SelectorOutbound {
        outbounds: vec![],
        default: "ghost".to_string(),
        interrupt_exist_connections: false,
    });
    let mut cfg2 = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["auto", "n1"], "default": "auto" }
        ]
    });
    apply_config_slices(&mut cfg2, &slices).unwrap();
    assert_eq!(
        cfg2["outbounds"][0]["default"], "auto",
        "dangling default ignored"
    );

    // 停用的内置条目不覆写。
    slices.outbounds.items[0].enabled = false;
    slices.outbounds.items[1].enabled = false;
    let mut cfg3 = json!({
        "outbounds": [
            { "type": "selector", "tag": "proxy", "outbounds": ["auto"], "default": "auto" },
            { "type": "urltest", "tag": "auto", "outbounds": [], "url": "https://www.gstatic.com/generate_204" }
        ]
    });
    apply_config_slices(&mut cfg3, &slices).unwrap();
    assert_eq!(cfg3["outbounds"][0]["default"], "auto");
    assert_eq!(
        cfg3["outbounds"][1]["url"],
        "https://www.gstatic.com/generate_204"
    );
}
