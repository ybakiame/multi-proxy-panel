use pp_common::{CoreType, PanelError, PanelResult, ProtocolType};
use serde_json::{Value, json};

use super::builder::{ConfigBuilder, InboundConfig};

/// sing-box configuration builder.
pub struct SingBoxConfigBuilder;

impl ConfigBuilder for SingBoxConfigBuilder {
    fn core_type(&self) -> CoreType {
        CoreType::SingBox
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
            ProtocolType::Hysteria2 => build_hysteria2_inbound(settings, tls),
            ProtocolType::TuicV5 => build_tuic_inbound(settings, tls),
            ProtocolType::Anytls => build_anytls_inbound(settings, tls),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut sb_inbounds = Vec::with_capacity(inbounds.len());
        for inbound in inbounds {
            sb_inbounds.push(self.build_inbound(
                inbound.protocol,
                &inbound.settings,
                inbound.tls.as_ref(),
            )?);
        }

        let (services, experimental) = build_api_services();

        let mut config = json!({
            "log": {
                "level": "warning",
                "timestamp": true
            },
            "inbounds": sb_inbounds,
            "outbounds": [
                {
                    "type": "direct",
                    "tag": "direct"
                }
            ],
            "route": {
                "auto_detect_interface": true
            }
        });

        if let Some(services_arr) = services.as_array() {
            if !services_arr.is_empty() {
                config["services"] = services;
            }
        }
        if let Some(exp_obj) = experimental.as_object() {
            if !exp_obj.is_empty() {
                config["experimental"] = experimental;
            }
        }

        Ok(config)
    }
}

/// Build the sing-box API service definitions.
/// Returns `(services, experimental)` to be merged into the top-level config.
fn build_api_services() -> (Value, Value) {
    let grpc_listen = std::env::var("PROXYPANEL_SINGBOX_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9092".to_string());
    let http_listen = std::env::var("PROXYPANEL_SINGBOX_HTTP_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let secret = std::env::var("PROXYPANEL_SINGBOX_API_SECRET").unwrap_or_default();

    let mut api_service = json!({
        "type": "api",
        "listen": grpc_listen,
    });
    if !secret.is_empty() {
        api_service["secret"] = serde_json::json!(secret);
    }

    let mut clash_api = json!({
        "external_controller": http_listen,
    });
    if !secret.is_empty() {
        clash_api["secret"] = serde_json::json!(secret);
    }

    (
        json!([api_service]),
        json!({
            "clash_api": clash_api,
        }),
    )
}

fn build_vless_inbound(
    protocol: ProtocolType,
    settings: &Value,
    tls: Option<&Value>,
) -> PanelResult<Value> {
    if protocol == ProtocolType::VlessXhttp {
        return Err(PanelError::Config(
            "sing-box does not support VLESS + XHTTP transport".into(),
        ));
    }

    let users = vless_clients_to_users(settings);
    let flow = settings.get("flow").and_then(|v| v.as_str()).unwrap_or("");

    let mut inbound = json!({
        "type": "vless",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("vless-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
    });

    if !flow.is_empty() {
        // Apply flow to all users
        if let Some(arr) = inbound["users"].as_array_mut() {
            for user in arr.iter_mut() {
                user["flow"] = json!(flow);
            }
        }
    }

    if protocol == ProtocolType::VlessReality {
        let reality_tls = build_singbox_reality_tls(settings, tls)
            .ok_or_else(|| PanelError::Validation(
                "VLESS+REALITY requires reality_dest and reality_private_key".into()
            ))?;
        inbound["tls"] = reality_tls;
    } else if let Some(tls_cfg) = tls {
        inbound["tls"] = json!({
            "enabled": true,
            "server_name": tls_cfg.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
            "certificate_path": tls_cfg.get("certFile").and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls_cfg.get("keyFile").and_then(|v| v.as_str()).unwrap_or(""),
        });
    }

    Ok(inbound)
}

/// Convert VLESS clients array (id, email, flow) to sing-box users array (uuid, name, flow).
fn vless_clients_to_users(settings: &Value) -> Value {
    if let Some(clients) = settings.get("clients").and_then(|v| v.as_array()) {
        let users: Vec<Value> = clients
            .iter()
            .map(|c| {
                let mut user = json!({
                    "uuid": c.get("id").and_then(|v| v.as_str()).unwrap_or(""),
                    "name": c.get("email").and_then(|v| v.as_str()).unwrap_or(""),
                });
                if let Some(flow) = c.get("flow").and_then(|v| v.as_str()) {
                    if !flow.is_empty() {
                        user["flow"] = json!(flow);
                    }
                }
                user
            })
            .collect();
        json!(users)
    } else {
        // Fallback: single user from uuid
        let uuid = settings.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        if uuid.is_empty() {
            json!([])
        } else {
            json!([{"uuid": uuid, "name": ""}])
        }
    }
}

/// Build sing-box TLS object with REALITY from neutral settings fields.
fn build_singbox_reality_tls(settings: &Value, _tls: Option<&Value>) -> Option<Value> {
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

    // Parse dest into server and port
    let (server, server_port) = if let Some((host, port_str)) = dest.rsplit_once(':') {
        let port = port_str.parse::<u64>().unwrap_or(443);
        (host.to_string(), port)
    } else {
        (dest.to_string(), 443)
    };

    let server_names_str = settings
        .get("reality_server_names")
        .and_then(|v| v.as_str())
        .or_else(|| settings.get("server_names").and_then(|v| v.as_str()))
        .unwrap_or("");
    let server_name = server_names_str
        .split(',')
        .next()
        .map(|s| s.trim())
        .unwrap_or(&server);

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

    let tls_obj = json!({
        "enabled": true,
        "server_name": server_name,
        "reality": {
            "enabled": true,
            "handshake": {
                "server": server,
                "server_port": server_port,
            },
            "private_key": private_key,
            "short_id": if short_ids.is_empty() { json!([""]) } else { json!(short_ids) },
        }
    });

    Some(tls_obj)
}

fn build_vmess_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let mut inbound = json!({
        "type": "vmess",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("vmess-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
    });

    if let Some(tls_cfg) = tls {
        inbound["tls"] = json!({
            "enabled": true,
            "server_name": tls_cfg.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
        });
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
        "type": "trojan",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("trojan-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": settings.get("clients").cloned().unwrap_or(json!([])),
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        }
    }))
}

fn build_shadowsocks_inbound(settings: &Value, _tls: Option<&Value>) -> PanelResult<Value> {
    Ok(json!({
        "type": "shadowsocks",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("ss-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(8388),
        "method": settings.get("method").and_then(|v| v.as_str()).unwrap_or("2022-blake3-aes-128-gcm"),
        "password": settings.get("password").and_then(|v| v.as_str()).unwrap_or(""),
        "network": settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp")
    }))
}

fn build_hysteria2_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "Hysteria2 requires TLS configuration".into(),
        ));
    }

    let users = password_clients_to_users(settings);
    let obfs_type = settings
        .get("obfs_type")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let obfs_password = settings
        .get("obfs_password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut inbound = json!({
        "type": "hysteria2",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("hy2-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        },
        "up_mbps": settings.get("up_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
        "down_mbps": settings.get("down_mbps").and_then(|v| v.as_u64()).unwrap_or(100),
    });

    if obfs_type != "none" && !obfs_password.is_empty() {
        inbound["obfs"] = json!({
            "type": obfs_type,
            "password": obfs_password,
        });
    }

    Ok(inbound)
}

fn build_anytls_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "AnyTLS requires TLS configuration".into(),
        ));
    }

    let users = password_clients_to_users(settings);
    let mut inbound = json!({
        "type": "anytls",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("anytls-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        },
    });

    if let Some(masquerade) = settings.get("masquerade").and_then(|v| v.as_str()) {
        if !masquerade.is_empty() {
            inbound["masquerade"] = json!(masquerade);
        }
    }

    Ok(inbound)
}

fn build_tuic_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    if tls.is_none() {
        return Err(PanelError::Validation(
            "TUIC requires TLS configuration".into(),
        ));
    }

    let users = tuic_clients_to_users(settings);

    Ok(json!({
        "type": "tuic",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("tuic-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
        "tls": {
            "enabled": true,
            "certificate_path": tls.and_then(|t| t.get("certFile")).and_then(|v| v.as_str()).unwrap_or(""),
            "key_path": tls.and_then(|t| t.get("keyFile")).and_then(|v| v.as_str()).unwrap_or(""),
        },
        "congestion_control": settings.get("congestion_control").and_then(|v| v.as_str()).unwrap_or("bbr"),
    }))
}

/// Convert clients array with password auth to sing-box users array (name, password).
fn password_clients_to_users(settings: &Value) -> Value {
    if let Some(clients) = settings.get("clients").and_then(|v| v.as_array()) {
        let users: Vec<Value> = clients
            .iter()
            .map(|c| {
                json!({
                    "name": c.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                    "password": c.get("password").and_then(|v| v.as_str()).unwrap_or(""),
                })
            })
            .collect();
        json!(users)
    } else {
        // Fallback: single user from settings.password
        let password = settings
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if password.is_empty() {
            json!([])
        } else {
            json!([{"name": "", "password": password}])
        }
    }
}

/// Convert clients array with UUID+password auth to sing-box TUIC users array (name, uuid, password).
fn tuic_clients_to_users(settings: &Value) -> Value {
    if let Some(clients) = settings.get("clients").and_then(|v| v.as_array()) {
        let users: Vec<Value> = clients
            .iter()
            .map(|c| {
                let mut user = json!({
                    "name": c.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                });
                if let Some(uuid) = c.get("uuid").and_then(|v| v.as_str()) {
                    if !uuid.is_empty() {
                        user["uuid"] = json!(uuid);
                    }
                }
                if let Some(password) = c.get("password").and_then(|v| v.as_str()) {
                    if !password.is_empty() {
                        user["password"] = json!(password);
                    }
                }
                user
            })
            .collect();
        json!(users)
    } else {
        let uuid = settings.get("uuid").and_then(|v| v.as_str()).unwrap_or("");
        let password = settings
            .get("password")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let mut user = json!({"name": ""});
        if !uuid.is_empty() {
            user["uuid"] = json!(uuid);
        }
        if !password.is_empty() {
            user["password"] = json!(password);
        }
        json!([user])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reality_settings() -> Value {
        json!({
            "tag": "vless-reality-in",
            "listen": "::",
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
    fn vless_reality_inbound_has_reality_tls() {
        let builder = SingBoxConfigBuilder;
        let inbound = builder
            .build_inbound(ProtocolType::VlessReality, &reality_settings(), None)
            .unwrap();

        assert_eq!(inbound["type"], "vless");
        assert_eq!(inbound["listen_port"], 443);
        assert_eq!(inbound["tls"]["enabled"], true);
        assert_eq!(inbound["tls"]["reality"]["enabled"], true);
        assert_eq!(inbound["tls"]["reality"]["handshake"]["server"], "example.com");
        assert_eq!(inbound["tls"]["reality"]["handshake"]["server_port"], 443);
        assert_eq!(
            inbound["tls"]["reality"]["private_key"],
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        );
        let users = inbound["users"].as_array().unwrap();
        assert_eq!(users[0]["uuid"], "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4");
        assert_eq!(users[0]["flow"], "xtls-rprx-vision");
    }

    #[test]
    fn vless_reality_requires_reality_private_key() {
        let builder = SingBoxConfigBuilder;
        let mut settings = reality_settings();
        settings["reality_private_key"] = "".into();
        settings["private_key"] = "".into();
        settings["reality_dest"] = "".into();

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
    fn full_config_contains_api_services_and_outbounds() {
        let builder = SingBoxConfigBuilder;
        let inbound = InboundConfig {
            tag: "vless-reality-in".into(),
            protocol: ProtocolType::VlessReality,
            listen: "::".into(),
            port: 443,
            settings: reality_settings(),
            tls: None,
            sniffing: None,
        };

        let config = builder.build_full_config(&[inbound]).unwrap();
        assert!(config["inbounds"].is_array());
        assert!(config["services"].is_array());
        assert_eq!(config["services"][0]["type"], "api");
        assert!(config["experimental"]["clash_api"].is_object());
        assert!(config["experimental"]["clash_api"]["external_controller"].is_string());
        assert_eq!(config["outbounds"][0]["type"], "direct");
    }
}
