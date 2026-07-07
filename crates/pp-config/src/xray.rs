use pp_common::{CoreType, PanelError, PanelResult, ProtocolType};
use serde_json::{Value, json};

use super::builder::{ConfigBuilder, InboundConfig};

/// Xray-core configuration builder.
pub struct XrayConfigBuilder;

impl ConfigBuilder for XrayConfigBuilder {
    fn core_type(&self) -> CoreType {
        CoreType::Xray
    }

    fn build_inbound(
        &self,
        protocol: ProtocolType,
        settings: &Value,
        tls: Option<&Value>,
    ) -> PanelResult<Value> {
        match protocol {
            ProtocolType::VlessReality | ProtocolType::VlessVision | ProtocolType::VlessXhttp => {
                build_vless_inbound(protocol, settings, tls)
            }
            ProtocolType::Vmess => build_vmess_inbound(settings, tls),
            ProtocolType::Trojan => build_trojan_inbound(settings, tls),
            ProtocolType::Shadowsocks2022 => build_shadowsocks_inbound(settings, tls),
            _ => Err(PanelError::Config(format!(
                "protocol {:?} not supported by xray",
                protocol
            ))),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut xray_inbounds = Vec::with_capacity(inbounds.len());
        for inbound in inbounds {
            xray_inbounds.push(self.build_inbound(
                inbound.protocol,
                &inbound.settings,
                inbound.tls.as_ref(),
            )?);
        }

        let routing = build_xray_routing();

        Ok(json!({
            "log": {
                "loglevel": "warning"
            },
            "inbounds": xray_inbounds,
            "outbounds": [
                {
                    "protocol": "freedom",
                    "tag": "direct"
                },
                {
                    "protocol": "blackhole",
                    "tag": "block"
                }
            ],
            "routing": routing,
        }))
    }
}

fn build_xray_routing() -> Value {
    let mut rules = Vec::<Value>::new();

    // Block BitTorrent
    rules.push(json!({
        "type": "field",
        "protocol": ["bittorrent"],
        "outboundTag": "block"
    }));

    // Block common AD domains (minimal list, user can override via base_config)
    rules.push(json!({
        "type": "field",
        "domain": [
            "geosite:category-ads-all"
        ],
        "outboundTag": "block"
    }));

    json!({
        "domainStrategy": "IPIfNonMatch",
        "rules": rules,
    })
}

fn build_vless_inbound(
    protocol: ProtocolType,
    settings: &Value,
    tls: Option<&Value>,
) -> PanelResult<Value> {
    // Build clients array: prefer settings.clients, fallback to single user from uuid/flow
    let clients = if let Some(clients_arr) = settings.get("clients").and_then(|v| v.as_array()) {
        json!(clients_arr)
    } else {
        let uuid = settings.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        let flow = settings.get("flow").and_then(|v| v.as_str()).unwrap_or("");
        if uuid.is_empty() {
            json!([])
        } else {
            let mut client = json!({ "id": uuid });
            if !flow.is_empty() {
                client["flow"] = json!(flow);
            }
            json!([client])
        }
    };

    let network = settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| stream_settings_network(&protocol));

    let mut inbound = json!({
        "protocol": "vless",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": clients,
            "decryption": "none"
        },
        "streamSettings": {
            "network": network,
            "security": if tls.is_some() && protocol != ProtocolType::VlessReality { "tls" } else { "none" },
        },
        "sniffing": {
            "enabled": true,
            "destOverride": ["http", "tls", "quic"]
        }
    });

    // Apply transport-specific stream settings
    apply_xray_transport_settings(&mut inbound, network, settings)?;

    match protocol {
        ProtocolType::VlessReality => {
            inbound["streamSettings"]["security"] = "reality".into();
            let reality_cfg = build_xray_reality_settings(settings).ok_or_else(|| {
                PanelError::Validation(
                    "VLESS+REALITY requires reality_dest and reality_private_key".into(),
                )
            })?;
            inbound["streamSettings"]["realitySettings"] = reality_cfg;
            // REALITY uses its own handshake; do not merge traditional TLS settings.
        }
        _ => {
            if let Some(tls_cfg) = tls {
                inbound["streamSettings"]["security"] = "tls".into();
                inbound["streamSettings"]["tlsSettings"] = tls_cfg.clone();
            }
        }
    }

    Ok(inbound)
}

fn apply_xray_transport_settings(
    inbound: &mut Value,
    network: &str,
    settings: &Value,
) -> PanelResult<()> {
    let stream = inbound
        .get_mut("streamSettings")
        .ok_or_else(|| PanelError::Config("missing streamSettings".into()))?;

    match network {
        "tcp" => {
            // TCP is the default; optionally accept HTTP camouflage settings.
            if let Some(tcp_header) = settings.get("tcp_header") {
                stream["tcpSettings"] = json!({ "header": tcp_header });
            }
        }
        "ws" => {
            let path = settings
                .get("path")
                .and_then(|v| v.as_str())
                .or_else(|| settings.get("ws_path").and_then(|v| v.as_str()))
                .unwrap_or("/");
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .or_else(|| settings.get("ws_host").and_then(|v| v.as_str()))
                .unwrap_or("");
            let mut ws = json!({ "path": path });
            if !host.is_empty() {
                ws["headers"] = json!({ "Host": host });
            }
            stream["wsSettings"] = ws;
        }
        "grpc" => {
            let service_name = settings
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            stream["grpcSettings"] = json!({
                "serviceName": service_name,
            });
        }
        "xhttp" | "httpupgrade" => {
            if let Some(xhttp_cfg) = build_xray_xhttp_settings(settings) {
                stream["xhttpSettings"] = xhttp_cfg;
            }
        }
        "kcp" | "mkcp" => {
            let mtu = settings
                .get("kcp_mtu")
                .and_then(|v| v.as_u64())
                .unwrap_or(1350);
            let tti = settings
                .get("kcp_tti")
                .and_then(|v| v.as_u64())
                .unwrap_or(50);
            let uplink_capacity = settings
                .get("kcp_uplink_capacity")
                .and_then(|v| v.as_u64())
                .unwrap_or(5);
            let downlink_capacity = settings
                .get("kcp_downlink_capacity")
                .and_then(|v| v.as_u64())
                .unwrap_or(20);
            let congestion = settings
                .get("kcp_congestion")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let seed = settings
                .get("seed")
                .and_then(|v| v.as_str())
                .or_else(|| settings.get("kcp_seed").and_then(|v| v.as_str()))
                .unwrap_or("");
            let header_type = settings
                .get("header_type")
                .and_then(|v| v.as_str())
                .unwrap_or("none");

            let mut kcp = json!({
                "mtu": mtu,
                "tti": tti,
                "uplinkCapacity": uplink_capacity,
                "downlinkCapacity": downlink_capacity,
                "congestion": congestion,
                "header": { "type": header_type },
            });
            if !seed.is_empty() {
                kcp["seed"] = json!(seed);
            }
            stream["kcpSettings"] = kcp;
        }
        _ => {}
    }

    Ok(())
}

fn build_xray_reality_settings(settings: &Value) -> Option<Value> {
    let dest = settings
        .get("reality_dest")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("dest").and_then(|v| v.as_str()))?;
    if dest.is_empty() {
        return None;
    }
    let private_key = settings
        .get("reality_private_key")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("private_key").and_then(|v| v.as_str()))?;
    if private_key.is_empty() {
        return None;
    }

    let server_names_str = settings
        .get("reality_server_names")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("server_names").and_then(|v| v.as_str()))
        .unwrap_or("");
    let server_names: Vec<String> = server_names_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let short_id_str = settings
        .get("reality_short_id")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("short_id").and_then(|v| v.as_str()))
        .unwrap_or("");
    let short_ids: Vec<String> = short_id_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let reality = json!({
        "dest": dest,
        "serverNames": if server_names.is_empty() { json!([""]) } else { json!(server_names) },
        "privateKey": private_key,
        "shortIds": if short_ids.is_empty() { json!([""]) } else { json!(short_ids) },
    });

    Some(reality)
}

fn build_xray_xhttp_settings(settings: &Value) -> Option<Value> {
    let path = settings
        .get("xhttp_path")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("path").and_then(|v| v.as_str()))?;
    let host = settings
        .get("xhttp_host")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("host").and_then(|v| v.as_str()))
        .unwrap_or("");
    let mode = settings
        .get("xhttp_mode")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("mode").and_then(|v| v.as_str()))
        .unwrap_or("auto");

    let mut xhttp = json!({
        "path": path,
        "mode": mode,
    });
    if !host.is_empty() {
        xhttp["host"] = json!(host);
    }

    Some(xhttp)
}

fn build_vmess_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let network = settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut inbound = json!({
        "protocol": "vmess",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([]))
        },
        "streamSettings": {
            "network": network,
            "security": if tls.is_some() { "tls" } else { "none" },
        },
        "sniffing": {
            "enabled": true,
            "destOverride": ["http", "tls", "quic"]
        }
    });

    apply_xray_transport_settings(&mut inbound, network, settings)?;

    if let Some(tls_cfg) = tls {
        inbound["streamSettings"]["tlsSettings"] = tls_cfg.clone();
    }

    Ok(inbound)
}

fn build_trojan_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "Trojan requires TLS configuration".into(),
        ));
    }

    let network = settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut inbound = json!({
        "protocol": "trojan",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([]))
        },
        "streamSettings": {
            "network": network,
            "security": "tls",
            "tlsSettings": tls.unwrap()
        }
    });

    apply_xray_transport_settings(&mut inbound, network, settings)?;

    Ok(inbound)
}

fn build_shadowsocks_inbound(settings: &Value, _tls: Option<&Value>) -> PanelResult<Value> {
    Ok(json!({
        "protocol": "shadowsocks",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(8388),
        "settings": {
            "method": settings.get("method").and_then(|v| v.as_str()).unwrap_or("2022-blake3-aes-128-gcm"),
            "password": settings.get("password").and_then(|v| v.as_str()).unwrap_or(""),
            "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp,udp")
        }
    }))
}

fn stream_settings_network(protocol: &ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::VlessReality | ProtocolType::VlessVision => "tcp",
        ProtocolType::VlessXhttp => "xhttp",
        _ => "tcp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reality_settings() -> Value {
        json!({
            "tag": "vless-reality-in",
            "listen": "0.0.0.0",
            "port": 443,
            "clients": [
                { "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4", "email": "alice@example.com", "flow": "xtls-rprx-vision" }
            ],
            "reality_dest": "example.com:443",
            "reality_server_names": "example.com,www.example.com",
            "reality_private_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "reality_short_id": "0123456789abcdef"
        })
    }

    #[test]
    fn vless_reality_inbound_has_reality_settings() {
        let builder = XrayConfigBuilder;
        let inbound = builder
            .build_inbound(ProtocolType::VlessReality, &reality_settings(), None)
            .unwrap();

        assert_eq!(inbound["protocol"], "vless");
        assert_eq!(inbound["port"], 443);
        assert_eq!(inbound["streamSettings"]["network"], "tcp");
        assert_eq!(inbound["streamSettings"]["security"], "reality");
        assert!(inbound["streamSettings"]["realitySettings"].is_object());
        assert_eq!(
            inbound["streamSettings"]["realitySettings"]["dest"],
            "example.com:443"
        );
        assert_eq!(
            inbound["streamSettings"]["realitySettings"]["privateKey"],
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        );
        let server_names = inbound["streamSettings"]["realitySettings"]["serverNames"]
            .as_array()
            .unwrap();
        assert_eq!(server_names.len(), 2);
        assert_eq!(server_names[0], "example.com");
    }

    #[test]
    fn vless_reality_requires_reality_private_key() {
        let builder = XrayConfigBuilder;
        let mut settings = reality_settings();
        settings["reality_private_key"] = "".into();
        settings["private_key"] = "".into();

        let err = builder
            .build_inbound(ProtocolType::VlessReality, &settings, None)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("reality_dest and reality_private_key"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn vless_ws_tls_inbound_has_ws_settings() {
        let builder = XrayConfigBuilder;
        let settings = json!({
            "tag": "vless-ws-in",
            "listen": "0.0.0.0",
            "port": 443,
            "clients": [
                { "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4", "email": "alice@example.com" }
            ],
            "network": "ws",
            "path": "/vless",
            "host": "cdn.example.com",
        });
        let tls = json!({ "serverName": "cdn.example.com" });
        let inbound = builder
            .build_inbound(ProtocolType::VlessVision, &settings, Some(&tls))
            .unwrap();

        assert_eq!(inbound["streamSettings"]["network"], "ws");
        assert_eq!(inbound["streamSettings"]["security"], "tls");
        assert_eq!(inbound["streamSettings"]["wsSettings"]["path"], "/vless");
        assert_eq!(
            inbound["streamSettings"]["wsSettings"]["headers"]["Host"],
            "cdn.example.com"
        );
    }

    #[test]
    fn vmess_grpc_inbound_has_grpc_settings() {
        let builder = XrayConfigBuilder;
        let settings = json!({
            "tag": "vmess-grpc-in",
            "listen": "0.0.0.0",
            "port": 443,
            "clients": [{ "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4" }],
            "network": "grpc",
            "service_name": "vmess-grpc",
        });
        let tls = json!({ "serverName": "example.com" });
        let inbound = builder
            .build_inbound(ProtocolType::Vmess, &settings, Some(&tls))
            .unwrap();

        assert_eq!(inbound["streamSettings"]["network"], "grpc");
        assert_eq!(
            inbound["streamSettings"]["grpcSettings"]["serviceName"],
            "vmess-grpc"
        );
    }

    #[test]
    fn full_config_contains_routing_rules() {
        let builder = XrayConfigBuilder;
        let inbound = InboundConfig {
            tag: "vless-reality-in".into(),
            protocol: ProtocolType::VlessReality,
            listen: "0.0.0.0".into(),
            port: 443,
            settings: reality_settings(),
            tls: None,
            sniffing: None,
        };

        let config = builder.build_full_config(&[inbound]).unwrap();
        assert!(config["routing"].is_object());
        assert!(!config["routing"]["rules"].as_array().unwrap().is_empty());
        assert_eq!(config["outbounds"][1]["tag"], "block");
    }
}
