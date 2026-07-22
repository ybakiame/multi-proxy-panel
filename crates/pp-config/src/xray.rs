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
            ProtocolType::VlessReality | ProtocolType::VlessXhttp => {
                build_vless_inbound(protocol, settings, tls)
            }
            _ => Err(PanelError::Config(format!(
                "protocol {:?} not supported by xray",
                protocol
            ))),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut xray_inbounds = Vec::with_capacity(inbounds.len() + 1);
        for inbound in inbounds {
            xray_inbounds.push(self.build_inbound(
                inbound.protocol,
                &inbound.settings,
                inbound.tls.as_ref(),
            )?);
        }

        let (api_addr, api_port) = xray_api_listen();
        xray_inbounds.push(json!({
            "tag": "api",
            "protocol": "dokodemo-door",
            "listen": api_addr,
            "port": api_port,
            "settings": {
                "address": api_addr
            }
        }));

        let routing = build_xray_routing();

        Ok(json!({
            "log": {
                "loglevel": "warning"
            },
            "api": {
                "tag": "api",
                "services": ["StatsService"]
            },
            "stats": {},
            "policy": {
                "levels": {
                    "0": {
                        "statsUserUplink": true,
                        "statsUserDownlink": true
                    }
                },
                "system": {
                    "statsInboundUplink": true,
                    "statsInboundDownlink": true,
                    "statsUserUplink": true,
                    "statsUserDownlink": true
                }
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

/// StatsService gRPC listen address for the xray `api` inbound.
///
/// Read on the Hub at config-generation time; the Agent reads the same
/// variable when querying stats, so both sides must agree.
fn xray_api_listen() -> (String, u16) {
    let listen = std::env::var("PROXYPANEL_XRAY_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    match listen.rsplit_once(':') {
        Some((addr, port)) => (addr.to_string(), port.parse().unwrap_or(8080)),
        None => (listen, 8080),
    }
}

fn build_xray_routing() -> Value {
    let mut rules = Vec::<Value>::new();

    // Route gRPC API requests hitting the dokodemo-door inbound to the api outbound tag.
    rules.push(json!({
        "type": "field",
        "inboundTag": ["api"],
        "outboundTag": "api"
    }));

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
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("vless-in"),
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
    // Xray requires an explicit host:port target; default to :443 when the
    // port is omitted (sing-box tolerates the same shorthand).
    let dest = if dest
        .rsplit_once(':')
        .and_then(|(_, p)| p.parse::<u16>().ok())
        .is_some()
    {
        dest.to_string()
    } else {
        format!("{}:443", dest)
    };
    let private_key = settings
        .get("reality_private_key")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("private_key").and_then(|v| v.as_str()))?;
    if private_key.is_empty() {
        return None;
    }

    let server_names = string_list_setting(settings, &["reality_server_names", "server_names"]);
    let short_ids = string_list_setting(settings, &["reality_short_id", "short_id"]);

    let reality = json!({
        "dest": dest,
        "serverNames": if server_names.is_empty() { json!([""]) } else { json!(server_names) },
        "privateKey": private_key,
        "shortIds": if short_ids.is_empty() { json!([""]) } else { json!(short_ids) },
    });

    Some(reality)
}

/// Read a setting that may be encoded either as a comma-separated string or
/// as a JSON array of strings, trying each key in order.
fn string_list_setting(settings: &Value, keys: &[&str]) -> Vec<String> {
    for key in keys {
        match settings.get(*key) {
            Some(Value::String(s)) => {
                return s
                    .split(',')
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect();
            }
            Some(Value::Array(arr)) => {
                return arr
                    .iter()
                    .filter_map(|v| v.as_str())
                    .map(|x| x.trim().to_string())
                    .filter(|x| !x.is_empty())
                    .collect();
            }
            _ => continue,
        }
    }
    Vec::new()
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

fn stream_settings_network(protocol: &ProtocolType) -> &'static str {
    match protocol {
        ProtocolType::VlessReality => "tcp",
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
    fn vless_reality_inbound_has_tag_and_reality_settings() {
        let builder = XrayConfigBuilder;
        let inbound = builder
            .build_inbound(ProtocolType::VlessReality, &reality_settings(), None)
            .unwrap();

        assert_eq!(inbound["tag"], "vless-reality-in");
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
    fn vless_reality_accepts_portless_dest_and_array_server_names() {
        let builder = XrayConfigBuilder;
        let settings = json!({
            "tag": "vless-reality-in",
            "port": 443,
            "clients": [{ "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4", "email": "alice@example.com" }],
            "dest": "www.samsung.com",
            "server_names": ["www.samsung.com"],
            "private_key": "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "short_id": "915d3f92"
        });
        let inbound = builder
            .build_inbound(ProtocolType::VlessReality, &settings, None)
            .unwrap();

        let reality = &inbound["streamSettings"]["realitySettings"];
        assert_eq!(reality["dest"], "www.samsung.com:443");
        assert_eq!(reality["serverNames"], json!(["www.samsung.com"]));
        assert_eq!(reality["shortIds"], json!(["915d3f92"]));
    }

    #[test]
    fn string_list_setting_accepts_string_and_array() {
        assert_eq!(
            string_list_setting(&json!({"a": "x, y"}), &["a"]),
            vec!["x".to_string(), "y".to_string()]
        );
        assert_eq!(
            string_list_setting(&json!({"a": ["x", "y"]}), &["a"]),
            vec!["x".to_string(), "y".to_string()]
        );
        assert!(string_list_setting(&json!({"b": 1}), &["a"]).is_empty());
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
    fn vless_xhttp_inbound_has_xhttp_settings() {
        let builder = XrayConfigBuilder;
        let settings = json!({
            "tag": "vless-xhttp-in",
            "listen": "0.0.0.0",
            "port": 443,
            "clients": [
                { "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4", "email": "alice@example.com" }
            ],
            "xhttp_path": "/xhttp",
            "xhttp_host": "cdn.example.com",
            "xhttp_mode": "auto",
        });
        let tls = json!({ "serverName": "cdn.example.com" });
        let inbound = builder
            .build_inbound(ProtocolType::VlessXhttp, &settings, Some(&tls))
            .unwrap();

        assert_eq!(inbound["streamSettings"]["network"], "xhttp");
        assert_eq!(inbound["streamSettings"]["security"], "tls");
        assert_eq!(inbound["streamSettings"]["xhttpSettings"]["path"], "/xhttp");
        assert_eq!(
            inbound["streamSettings"]["xhttpSettings"]["host"],
            "cdn.example.com"
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
            core_version: None,
        };

        let config = builder.build_full_config(&[inbound]).unwrap();
        assert!(config["routing"].is_object());
        assert!(!config["routing"]["rules"].as_array().unwrap().is_empty());
        assert_eq!(config["outbounds"][1]["tag"], "block");
    }

    #[test]
    fn full_config_enables_stats_service() {
        let builder = XrayConfigBuilder;
        let inbound = InboundConfig {
            tag: "vless-reality-in".into(),
            protocol: ProtocolType::VlessReality,
            listen: "0.0.0.0".into(),
            port: 443,
            settings: reality_settings(),
            tls: None,
            sniffing: None,
            core_version: None,
        };

        let config = builder.build_full_config(&[inbound]).unwrap();

        assert_eq!(config["api"]["tag"], "api");
        assert_eq!(config["api"]["services"], json!(["StatsService"]));
        assert!(config["stats"].is_object());
        assert_eq!(config["policy"]["system"]["statsInboundUplink"], true);
        assert_eq!(config["policy"]["system"]["statsInboundDownlink"], true);
        assert_eq!(config["policy"]["system"]["statsUserUplink"], true);
        assert_eq!(config["policy"]["system"]["statsUserDownlink"], true);

        let inbounds = config["inbounds"].as_array().unwrap();
        let api_inbound = inbounds
            .iter()
            .find(|i| i["tag"] == "api")
            .expect("api inbound missing");
        assert_eq!(api_inbound["protocol"], "dokodemo-door");
        assert_eq!(api_inbound["listen"], "127.0.0.1");

        let rules = config["routing"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["inboundTag"], json!(["api"]));
        assert_eq!(rules[0]["outboundTag"], "api");
    }
}
