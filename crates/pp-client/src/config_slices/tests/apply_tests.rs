//! Pure render + apply tests for config slices.
//!
//! Split out of `apply.rs` to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use crate::config_slices::*;
use serde_json::json;

fn dns_slice() -> DnsSlice {
    DnsSlice {
        enabled: true,
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
        }],
        final_tag: "remote".to_string(),
        strategy: DnsStrategy::PreferIpv6,
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
            enabled: true,
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
            enabled: true,
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
        outbounds: OutboundsSlice {
            enabled: true,
            items: vec![item],
        },
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
            enabled: true,
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
    };
    let slices = ConfigSlices {
        outbounds: OutboundsSlice {
            enabled: true,
            items: vec![group],
        },
        ..Default::default()
    };

    let report = apply_config_slices(&mut config, &slices).unwrap();
    assert!(report.outbound_tags.is_empty());
    assert_eq!(config["outbounds"].as_array().unwrap().len(), 1);
}
