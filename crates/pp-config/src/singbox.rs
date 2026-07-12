use pp_common::{CoreType, PanelError, PanelResult, ProtocolType};
use serde_json::{Value, json};
use tracing::warn;

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
            ProtocolType::VlessReality => build_vless_inbound(settings, tls),
            ProtocolType::Hysteria2 => build_hysteria2_inbound(settings, tls),
            ProtocolType::Anytls => build_anytls_inbound(settings, tls),
            _ => Err(PanelError::Config(format!(
                "protocol {:?} not supported by sing-box",
                protocol
            ))),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut sb_inbounds = Vec::with_capacity(inbounds.len());
        for inbound in inbounds {
            match self.build_inbound(inbound.protocol, &inbound.settings, inbound.tls.as_ref()) {
                Ok(value) => sb_inbounds.push(value),
                Err(e) => {
                    let err_msg = e.to_string();
                    if matches!(
                        inbound.protocol,
                        ProtocolType::Hysteria2 | ProtocolType::Anytls
                    ) && err_msg.contains("TLS")
                    {
                        warn!("skipping sing-box inbound {}: {}", inbound.tag, err_msg);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        let effective_version = effective_singbox_version(inbounds);
        let use_new_api = version_gte(&effective_version, "1.14.0");
        let route = build_singbox_route();

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
                },
                {
                    "type": "block",
                    "tag": "block"
                }
            ],
            "route": route
        });

        if use_new_api {
            let services = build_api_service();
            if let Some(arr) = services.as_array() {
                if !arr.is_empty() {
                    config["services"] = services;
                }
            }
        } else {
            let experimental = build_legacy_api_services();
            if let Some(exp_obj) = experimental.as_object() {
                if !exp_obj.is_empty() {
                    config["experimental"] = experimental;
                }
            }
        }

        Ok(config)
    }
}

fn build_singbox_route() -> Value {
    json!({
        "auto_detect_interface": true,
        "rules": [
            {
                "protocol": "bittorrent",
                "outbound": "block"
            }
        ]
    })
}

fn effective_singbox_version(inbounds: &[InboundConfig]) -> String {
    let requested: Vec<&str> = inbounds
        .iter()
        .filter_map(|i| i.core_version.as_deref())
        .filter(|v| !v.is_empty())
        .collect();

    if requested.is_empty() {
        return "1.14.0".to_string();
    }

    requested
        .into_iter()
        .max_by(|a, b| compare_versions(a, b))
        .unwrap_or("1.14.0")
        .to_string()
}

fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse(v: &str) -> Vec<u32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect()
    }
    parse(a).cmp(&parse(b))
}

fn version_gte(version: &str, target: &str) -> bool {
    compare_versions(version, target) != std::cmp::Ordering::Less
}

/// Build the sing-box 1.14.0+ API service definition.
fn build_api_service() -> Value {
    let http_listen = std::env::var("PROXYPANEL_SINGBOX_HTTP_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let secret = std::env::var("PROXYPANEL_SINGBOX_API_SECRET").unwrap_or_default();

    // sing-box 1.14.0-alpha expects `listen` to be an IP address only
    // (the port is fixed by the implementation).
    let listen_addr = http_listen
        .rsplit_once(':')
        .map(|(a, _)| a)
        .unwrap_or(&http_listen);

    let mut api = json!({
        "type": "api",
        "listen": listen_addr,
    });
    if !secret.is_empty() {
        api["secret"] = serde_json::json!(secret);
    }

    json!([api])
}

/// Build the legacy sing-box experimental clash_api definition (pre-1.14.0).
fn build_legacy_api_services() -> Value {
    let http_listen = std::env::var("PROXYPANEL_SINGBOX_HTTP_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let secret = std::env::var("PROXYPANEL_SINGBOX_API_SECRET").unwrap_or_default();

    let mut clash_api = json!({
        "external_controller": http_listen,
    });
    if !secret.is_empty() {
        clash_api["secret"] = serde_json::json!(secret);
    }

    json!({
        "clash_api": clash_api,
    })
}

fn build_vless_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let users = vless_clients_to_users(settings);
    let flow = settings.get("flow").and_then(|v| v.as_str()).unwrap_or("");
    let network = settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut inbound = json!({
        "type": "vless",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("vless-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
    });

    let transport = build_singbox_transport(network, settings);
    if !transport.is_null() {
        inbound["transport"] = transport;
    }

    if !flow.is_empty() {
        // Apply flow to all users
        if let Some(arr) = inbound["users"].as_array_mut() {
            for user in arr.iter_mut() {
                user["flow"] = json!(flow);
            }
        }
    }

    let reality_tls = build_singbox_reality_tls(settings, tls).ok_or_else(|| {
        PanelError::Validation("VLESS+REALITY requires reality_dest and reality_private_key".into())
    })?;
    inbound["tls"] = reality_tls;

    Ok(inbound)
}

/// Build sing-box transport object from neutral transport settings.
fn build_singbox_transport(network: &str, settings: &Value) -> Value {
    match network {
        "ws" => {
            let path = settings.get("path").and_then(|v| v.as_str()).unwrap_or("/");
            let host = settings.get("host").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "type": "ws",
                "path": path,
                "headers": if host.is_empty() { json!({}) } else { json!({ "Host": host }) },
            })
        }
        "grpc" => {
            let service_name = settings
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            json!({
                "type": "grpc",
                "service_name": service_name,
            })
        }
        "httpupgrade" | "xhttp" => {
            let path = settings
                .get("path")
                .and_then(|v| v.as_str())
                .or_else(|| settings.get("xhttp_path").and_then(|v| v.as_str()))
                .unwrap_or("/");
            let host = settings
                .get("host")
                .and_then(|v| v.as_str())
                .or_else(|| settings.get("xhttp_host").and_then(|v| v.as_str()))
                .unwrap_or("");
            json!({
                "type": "httpupgrade",
                "path": path,
                "host": host,
            })
        }
        _ => serde_json::Value::Null,
    }
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

    let server_name =
        pp_common::settings_helper::first_server_name(settings).unwrap_or_else(|| server.clone());

    let short_ids: Vec<String> = pp_common::settings_helper::first_short_id(settings)
        .map(|id| vec![id])
        .unwrap_or_else(|| {
            settings
                .get("reality_short_id")
                .or_else(|| settings.get("short_id"))
                .and_then(|v| v.as_str())
                .map(|s| {
                    s.split(',')
                        .map(|x| x.trim().to_string())
                        .filter(|x| !x.is_empty())
                        .collect()
                })
                .unwrap_or_default()
        });

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

fn build_singbox_server_tls(tls: Option<&Value>) -> PanelResult<Value> {
    let tls = tls.ok_or_else(|| PanelError::Validation("TLS configuration is required".into()))?;

    // User-provided certificate files
    let cert_file = tls.get("certFile").and_then(|v| v.as_str()).unwrap_or("");
    let key_file = tls.get("keyFile").and_then(|v| v.as_str()).unwrap_or("");
    if !cert_file.is_empty() && !key_file.is_empty() {
        return Ok(json!({
            "enabled": true,
            "certificate_path": cert_file,
            "key_path": key_file,
        }));
    }

    // ACME automatic certificate
    let domain = tls.get("domain").and_then(|v| v.as_str()).unwrap_or("");
    if !domain.is_empty() {
        return Ok(json!({
            "enabled": true,
            "acme": {
                "domain": [domain],
                "data_directory": "acme",
            },
        }));
    }

    Err(PanelError::Validation(
        "TLS requires certFile+keyFile or a domain for ACME".into(),
    ))
}

fn build_hysteria2_inbound(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let users = password_clients_to_users(settings);
    let obfs_type = settings
        .get("obfs_type")
        .and_then(|v| v.as_str())
        .unwrap_or("none");
    let obfs_password = settings
        .get("obfs_password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut tls_obj = build_singbox_server_tls(tls)?;
    tls_obj["alpn"] = json!(["h3"]);

    let mut inbound = json!({
        "type": "hysteria2",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("hy2-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
        "tls": tls_obj,
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
    let users = password_clients_to_users(settings);
    let mut inbound = json!({
        "type": "anytls",
        "tag": settings.get("tag").and_then(|v| v.as_str()).unwrap_or("anytls-in"),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("::"),
        "listen_port": settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443),
        "users": users,
        "tls": build_singbox_server_tls(tls)?,
    });

    if let Some(masquerade) = settings.get("masquerade").and_then(|v| v.as_str()) {
        if !masquerade.is_empty() {
            inbound["masquerade"] = json!(masquerade);
        }
    }

    Ok(inbound)
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
        assert_eq!(
            inbound["tls"]["reality"]["handshake"]["server"],
            "example.com"
        );
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
            err.to_string()
                .contains("reality_dest and reality_private_key"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn hysteria2_inbound_has_users_and_tls() {
        let builder = SingBoxConfigBuilder;
        let settings = json!({
            "tag": "hy2-in",
            "listen": "::",
            "port": 8444,
            "clients": [{ "name": "alice", "password": "secret" }],
        });
        let tls = json!({ "certFile": "/tmp/cert.pem", "keyFile": "/tmp/key.pem" });
        let inbound = builder
            .build_inbound(ProtocolType::Hysteria2, &settings, Some(&tls))
            .unwrap();

        assert_eq!(inbound["type"], "hysteria2");
        assert_eq!(inbound["listen_port"], 8444);
        assert_eq!(inbound["tls"]["enabled"], true);
        assert_eq!(inbound["tls"]["certificate_path"], "/tmp/cert.pem");
        assert_eq!(inbound["tls"]["key_path"], "/tmp/key.pem");
        assert_eq!(inbound["tls"]["alpn"], json!(["h3"]));
        let users = inbound["users"].as_array().unwrap();
        assert_eq!(users[0]["password"], "secret");
    }

    #[test]
    fn hysteria2_inbound_acme_tls() {
        let builder = SingBoxConfigBuilder;
        let settings = json!({
            "tag": "hy2-in",
            "listen": "::",
            "port": 8444,
            "clients": [{ "name": "alice", "password": "secret" }],
        });
        let tls = json!({ "domain": "hy2.example.com" });
        let inbound = builder
            .build_inbound(ProtocolType::Hysteria2, &settings, Some(&tls))
            .unwrap();

        assert_eq!(inbound["type"], "hysteria2");
        assert_eq!(inbound["tls"]["enabled"], true);
        let domains = inbound["tls"]["acme"]["domain"].as_array().unwrap();
        assert_eq!(domains[0], "hy2.example.com");
        assert_eq!(inbound["tls"]["acme"]["data_directory"], "acme");
        assert_eq!(inbound["tls"]["alpn"], json!(["h3"]));
    }

    #[test]
    fn full_config_skips_hysteria2_without_tls() {
        let builder = SingBoxConfigBuilder;
        let reality = InboundConfig {
            tag: "vless-reality-in".into(),
            protocol: ProtocolType::VlessReality,
            listen: "::".into(),
            port: 443,
            settings: reality_settings(),
            tls: None,
            sniffing: None,
            core_version: None,
        };
        let hysteria = InboundConfig {
            tag: "hy2-in".into(),
            protocol: ProtocolType::Hysteria2,
            listen: "::".into(),
            port: 8444,
            settings: json!({
                "tag": "hy2-in",
                "listen": "::",
                "port": 8444,
                "clients": [{ "name": "alice", "password": "secret" }],
            }),
            tls: None,
            sniffing: None,
            core_version: None,
        };

        let config = builder.build_full_config(&[reality, hysteria]).unwrap();
        let inbounds = config["inbounds"].as_array().unwrap();
        assert_eq!(inbounds.len(), 1);
        assert_eq!(inbounds[0]["type"], "vless");
    }

    #[test]
    fn full_config_contains_route_rules() {
        let builder = SingBoxConfigBuilder;
        let inbound = InboundConfig {
            tag: "vless-reality-in".into(),
            protocol: ProtocolType::VlessReality,
            listen: "::".into(),
            port: 443,
            settings: reality_settings(),
            tls: None,
            sniffing: None,
            core_version: None,
        };

        let config = builder.build_full_config(&[inbound]).unwrap();
        assert!(!config["route"]["rules"].as_array().unwrap().is_empty());
        assert_eq!(config["outbounds"][1]["tag"], "block");
    }
}
