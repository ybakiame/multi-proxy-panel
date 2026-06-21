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

        Ok(json!({
            "log": {
                "loglevel": "warning"
            },
            "inbounds": xray_inbounds,
            "outbounds": [
                {
                    "protocol": "freedom",
                    "tag": "direct"
                }
            ]
        }))
    }
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

    let mut inbound = json!({
        "protocol": "vless",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": clients,
            "decryption": "none"
        },
        "streamSettings": {
            "network": stream_settings_network(&protocol),
            "security": if tls.is_some() && protocol != ProtocolType::VlessReality { "tls" } else { "none" },
        },
        "sniffing": {
            "enabled": true,
            "destOverride": ["http", "tls", "quic"]
        }
    });

    match protocol {
        ProtocolType::VlessReality => {
            inbound["streamSettings"]["security"] = "reality".into();
            let reality_cfg = build_xray_reality_settings(settings)
                .ok_or_else(|| PanelError::Validation(
                    "VLESS+REALITY requires reality_dest and reality_private_key".into()
                ))?;
            inbound["streamSettings"]["realitySettings"] = reality_cfg;
            // REALITY uses its own handshake; do not merge traditional TLS settings.
        }
        ProtocolType::VlessVision => {
            if let Some(tls_cfg) = tls {
                inbound["streamSettings"]["security"] = "tls".into();
                inbound["streamSettings"]["tlsSettings"] = tls_cfg.clone();
            }
        }
        ProtocolType::VlessXhttp => {
            if let Some(xhttp_cfg) = build_xray_xhttp_settings(settings) {
                inbound["streamSettings"]["xhttpSettings"] = xhttp_cfg;
            }
            if let Some(tls_cfg) = tls {
                inbound["streamSettings"]["security"] = "tls".into();
                inbound["streamSettings"]["tlsSettings"] = tls_cfg.clone();
            }
        }
        _ => {}
    }

    Ok(inbound)
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
    let mut inbound = json!({
        "protocol": "vmess",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([]))
        },
        "streamSettings": {
            "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp"),
            "security": if tls.is_some() { "tls" } else { "none" },
        },
        "sniffing": {
            "enabled": true,
            "destOverride": ["http", "tls", "quic"]
        }
    });

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

    Ok(json!({
        "protocol": "trojan",
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        "port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "settings": {
            "clients": settings.get("clients").cloned().unwrap_or(json!([]))
        },
        "streamSettings": {
            "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp"),
            "security": "tls",
            "tlsSettings": tls.unwrap()
        }
    }))
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
            err.to_string().contains("reality_dest and reality_private_key"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn full_config_contains_inbounds_and_freedom_outbound() {
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
        assert!(config["inbounds"].is_array());
        assert_eq!(config["inbounds"].as_array().unwrap().len(), 1);
        assert_eq!(config["outbounds"][0]["protocol"], "freedom");
        assert_eq!(config["log"]["loglevel"], "warning");
    }
}
