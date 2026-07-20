use pp_common::{CoreType, PanelError, PanelResult, ProtocolType};
use serde_json::{Value, json};
use tracing::warn;

use super::builder::{ConfigBuilder, InboundConfig};

/// mihomo (Clash.Meta) configuration builder.
///
/// Emits config as JSON like the other builders; `MihomoProcessManager`
/// serializes it to YAML when writing `mihomo.yaml` (mihomo only accepts
/// YAML config files). Field names follow mihomo's listener schema:
/// users are a list for vless but a map for hysteria2/anytls, TLS uses
/// `certificate`/`private-key`, and vless has no `network` field — the
/// transport is selected by which sub-field (e.g. `xhttp-config`) is present.
pub struct MihomoConfigBuilder;

impl ConfigBuilder for MihomoConfigBuilder {
    fn core_type(&self) -> CoreType {
        CoreType::Mihomo
    }

    fn build_inbound(
        &self,
        protocol: ProtocolType,
        settings: &Value,
        tls: Option<&Value>,
    ) -> PanelResult<Value> {
        match protocol {
            ProtocolType::VlessReality => build_vless_reality_listener(settings),
            ProtocolType::VlessXhttp => build_vless_xhttp_listener(settings, tls),
            ProtocolType::Hysteria2 => build_hysteria2_listener(settings, tls),
            ProtocolType::Anytls => build_anytls_listener(settings, tls),
        }
    }

    fn build_full_config(&self, inbounds: &[InboundConfig]) -> PanelResult<Value> {
        let mut listeners = Vec::with_capacity(inbounds.len());
        for inbound in inbounds {
            match self.build_inbound(inbound.protocol, &inbound.settings, inbound.tls.as_ref()) {
                Ok(listener) => listeners.push(listener),
                Err(e) => {
                    let err_msg = e.to_string();
                    if matches!(
                        inbound.protocol,
                        ProtocolType::VlessXhttp | ProtocolType::Hysteria2 | ProtocolType::Anytls
                    ) && err_msg.contains("TLS")
                    {
                        warn!("skipping mihomo listener {}: {}", inbound.tag, err_msg);
                        continue;
                    }
                    return Err(e);
                }
            }
        }

        let mut config = json!({
            "log-level": "warning",
            "mode": "rule",
            "allow-lan": false,
            "external-controller": "127.0.0.1:9093",
            "listeners": listeners,
            "rules": ["MATCH,DIRECT"],
        });

        let secret = std::env::var("PROXYPANEL_MIHOMO_API_SECRET").unwrap_or_default();
        if !secret.is_empty() {
            config["secret"] = json!(secret);
        }

        Ok(config)
    }
}

fn listener_base(settings: &Value, default_name: &str) -> Value {
    let port = settings.get("port").and_then(|v| v.as_u64()).unwrap_or(443);
    json!({
        "name": settings.get("tag").and_then(|v| v.as_str()).unwrap_or(default_name),
        "listen": settings.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0"),
        // mihomo listener port is a string (supports range syntax like "200,302")
        "port": port.to_string(),
    })
}

fn setting_str<'a>(settings: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|k| settings.get(*k).and_then(|v| v.as_str()))
}

fn comma_list(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// vless users are a list of {username, uuid, flow?}; flow applies to
/// reality (vision) only, not xhttp.
fn vless_users(settings: &Value, with_flow: bool) -> Value {
    let default_flow = if with_flow {
        setting_str(settings, &["flow"]).unwrap_or("")
    } else {
        ""
    };

    let build_user = |uuid: &str, name: &str, flow: &str| {
        let mut user = json!({ "username": name, "uuid": uuid });
        let effective_flow = if flow.is_empty() { default_flow } else { flow };
        if with_flow && !effective_flow.is_empty() {
            user["flow"] = json!(effective_flow);
        }
        user
    };

    if let Some(clients) = settings.get("clients").and_then(|v| v.as_array()) {
        let users: Vec<Value> = clients
            .iter()
            .map(|c| {
                let uuid = setting_str(c, &["id", "uuid"]).unwrap_or("");
                let name = setting_str(c, &["email", "name"]).unwrap_or("");
                let flow = setting_str(c, &["flow"]).unwrap_or("");
                build_user(uuid, name, flow)
            })
            .collect();
        json!(users)
    } else {
        let uuid = setting_str(settings, &["uuid"]).unwrap_or("");
        if uuid.is_empty() {
            json!([])
        } else {
            json!([build_user(uuid, "", "")])
        }
    }
}

/// hysteria2/anytls users are a map of username -> password.
fn password_users(settings: &Value) -> Value {
    let mut map = serde_json::Map::new();

    if let Some(clients) = settings.get("clients").and_then(|v| v.as_array()) {
        for c in clients {
            let password = setting_str(c, &["password"]).unwrap_or("");
            if password.is_empty() {
                continue;
            }
            let name = setting_str(c, &["name", "email"]).unwrap_or("");
            let key = if name.is_empty() {
                format!("user{}", map.len() + 1)
            } else {
                name.to_string()
            };
            map.insert(key, json!(password));
        }
    } else {
        let password = setting_str(settings, &["password"]).unwrap_or("");
        if !password.is_empty() {
            map.insert("default".to_string(), json!(password));
        }
    }

    Value::Object(map)
}

/// sing-box 内置 ACME（lego）在共享数据目录下的落盘位置。agent 上
/// sing-box 与 mihomo 都以该数据目录为工作目录，因此 mihomo 可以用相对
/// 路径直接引用 sing-box 申请到的证书；mihomo v1.19.18+ 会监听证书文件
/// 变更并自动热加载，sing-box 续期后 mihomo 无需重启。
const ACME_CERT_DIR: &str = "acme/certificates/acme-v02.api.letsencrypt.org-directory";

/// mihomo TLS：优先使用显式证书文件；ACME 域名则引用 sing-box 内置 ACME
/// 在同一数据目录下已申请（或将会申请）的证书路径。
fn apply_cert_tls(listener: &mut Value, tls: Option<&Value>) -> PanelResult<()> {
    let tls = tls.ok_or_else(|| PanelError::Validation("TLS configuration is required".into()))?;

    let cert = setting_str(tls, &["certFile"]).unwrap_or("");
    let key = setting_str(tls, &["keyFile"]).unwrap_or("");
    if !cert.is_empty() && !key.is_empty() {
        listener["certificate"] = json!(cert);
        listener["private-key"] = json!(key);
        return Ok(());
    }

    let domain = setting_str(tls, &["domain"]).unwrap_or("");
    if !domain.is_empty() {
        let base = format!("{ACME_CERT_DIR}/{domain}/{domain}");
        listener["certificate"] = json!(format!("{}.crt", base));
        listener["private-key"] = json!(format!("{}.key", base));
        return Ok(());
    }

    Err(PanelError::Validation(
        "mihomo TLS requires certFile+keyFile or an ACME domain".into(),
    ))
}

fn build_vless_reality_listener(settings: &Value) -> PanelResult<Value> {
    let dest = setting_str(settings, &["reality_dest", "dest"])
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            PanelError::Validation(
                "VLESS+REALITY requires reality_dest and reality_private_key".into(),
            )
        })?;
    let private_key = setting_str(settings, &["reality_private_key", "private_key"])
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            PanelError::Validation(
                "VLESS+REALITY requires reality_dest and reality_private_key".into(),
            )
        })?;

    let mut server_names = comma_list(setting_str(
        settings,
        &["reality_server_names", "server_names"],
    ));
    if server_names.is_empty() {
        let host = dest.rsplit_once(':').map(|(h, _)| h).unwrap_or(dest);
        server_names.push(host.to_string());
    }
    let short_ids = comma_list(setting_str(settings, &["reality_short_id", "short_id"]));

    let mut listener = listener_base(settings, "vless-reality-in");
    listener["type"] = json!("vless");
    listener["users"] = vless_users(settings, true);
    listener["reality-config"] = json!({
        "dest": dest,
        "private-key": private_key,
        "server-names": server_names,
        "short-id": if short_ids.is_empty() { json!([""]) } else { json!(short_ids) },
    });
    Ok(listener)
}

fn build_vless_xhttp_listener(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let mut listener = listener_base(settings, "vless-xhttp-in");
    listener["type"] = json!("vless");
    listener["users"] = vless_users(settings, false);
    apply_cert_tls(&mut listener, tls)?;

    let path = setting_str(settings, &["xhttp_path", "path"]).unwrap_or("/");
    let host = setting_str(settings, &["xhttp_host", "host"]).unwrap_or("");
    let mode = setting_str(settings, &["xhttp_mode", "mode"]).unwrap_or("auto");
    listener["xhttp-config"] = json!({
        "path": path,
        "host": host,
        "mode": mode,
    });
    Ok(listener)
}

fn build_hysteria2_listener(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let mut listener = listener_base(settings, "hy2-in");
    listener["type"] = json!("hysteria2");
    listener["users"] = password_users(settings);
    apply_cert_tls(&mut listener, tls)?;
    listener["alpn"] = json!(["h3"]);

    let up = settings
        .get("up_mbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    let down = settings
        .get("down_mbps")
        .and_then(|v| v.as_u64())
        .unwrap_or(100);
    listener["up"] = json!(up.to_string());
    listener["down"] = json!(down.to_string());

    let obfs_type = setting_str(settings, &["obfs_type"]).unwrap_or("none");
    let obfs_password = setting_str(settings, &["obfs_password"]).unwrap_or("");
    if obfs_type != "none" && !obfs_password.is_empty() {
        listener["obfs"] = json!(obfs_type);
        listener["obfs-password"] = json!(obfs_password);
    }

    if let Some(masquerade) = setting_str(settings, &["masquerade"]).filter(|s| !s.is_empty()) {
        listener["masquerade"] = json!(masquerade);
    }

    Ok(listener)
}

fn build_anytls_listener(settings: &Value, tls: Option<&Value>) -> PanelResult<Value> {
    let mut listener = listener_base(settings, "anytls-in");
    listener["type"] = json!("anytls");
    listener["users"] = password_users(settings);
    apply_cert_tls(&mut listener, tls)?;
    Ok(listener)
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
    fn vless_reality_listener_has_reality_config() {
        let builder = MihomoConfigBuilder;
        let listener = builder
            .build_inbound(ProtocolType::VlessReality, &reality_settings(), None)
            .unwrap();

        assert_eq!(listener["type"], "vless");
        assert_eq!(listener["port"], "443");
        let reality = &listener["reality-config"];
        assert_eq!(reality["dest"], "example.com:443");
        assert_eq!(
            reality["private-key"],
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
        );
        assert_eq!(reality["server-names"][0], "example.com");
        assert_eq!(reality["short-id"][0], "0123456789abcdef");
        let users = listener["users"].as_array().unwrap();
        assert_eq!(users[0]["uuid"], "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4");
        assert_eq!(users[0]["username"], "alice@example.com");
        assert_eq!(users[0]["flow"], "xtls-rprx-vision");
    }

    #[test]
    fn vless_reality_requires_private_key() {
        let builder = MihomoConfigBuilder;
        let mut settings = reality_settings();
        settings["reality_private_key"] = "".into();
        settings["private_key"] = "".into();

        let err = builder
            .build_inbound(ProtocolType::VlessReality, &settings, None)
            .unwrap_err();
        assert!(
            err.to_string()
                .contains("reality_dest and reality_private_key")
        );
    }

    #[test]
    fn vless_xhttp_listener_has_xhttp_config_and_cert() {
        let builder = MihomoConfigBuilder;
        let settings = json!({
            "tag": "vless-xhttp-in",
            "listen": "0.0.0.0",
            "port": 8443,
            "clients": [{ "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4", "email": "bob@example.com" }],
            "xhttp_path": "/xhttp",
            "xhttp_host": "cdn.example.com",
            "xhttp_mode": "auto",
        });
        let tls = json!({ "certFile": "/tmp/cert.pem", "keyFile": "/tmp/key.pem" });
        let listener = builder
            .build_inbound(ProtocolType::VlessXhttp, &settings, Some(&tls))
            .unwrap();

        assert_eq!(listener["certificate"], "/tmp/cert.pem");
        assert_eq!(listener["private-key"], "/tmp/key.pem");
        assert_eq!(listener["xhttp-config"]["path"], "/xhttp");
        assert_eq!(listener["xhttp-config"]["host"], "cdn.example.com");
        assert_eq!(listener["xhttp-config"]["mode"], "auto");
        assert!(listener["users"][0].get("flow").is_none());
    }

    #[test]
    fn vless_xhttp_acme_domain_maps_to_shared_acme_cert_paths() {
        let builder = MihomoConfigBuilder;
        let settings = json!({ "tag": "t", "port": 443, "clients": [] });
        let tls = json!({ "domain": "hy2.example.com" });
        let listener = builder
            .build_inbound(ProtocolType::VlessXhttp, &settings, Some(&tls))
            .unwrap();

        let base = "acme/certificates/acme-v02.api.letsencrypt.org-directory/hy2.example.com/hy2.example.com";
        assert_eq!(listener["certificate"], format!("{}.crt", base));
        assert_eq!(listener["private-key"], format!("{}.key", base));
    }

    #[test]
    fn hysteria2_users_are_a_map() {
        let builder = MihomoConfigBuilder;
        let settings = json!({
            "tag": "hy2-in",
            "port": 8444,
            "clients": [{ "name": "alice", "password": "secret" }],
            "obfs_type": "salamander",
            "obfs_password": "obfspw",
            "up_mbps": 200,
            "down_mbps": 500,
        });
        let tls = json!({ "certFile": "/tmp/cert.pem", "keyFile": "/tmp/key.pem" });
        let listener = builder
            .build_inbound(ProtocolType::Hysteria2, &settings, Some(&tls))
            .unwrap();

        assert_eq!(listener["type"], "hysteria2");
        assert_eq!(listener["users"]["alice"], "secret");
        assert_eq!(listener["alpn"], json!(["h3"]));
        assert_eq!(listener["up"], "200");
        assert_eq!(listener["down"], "500");
        assert_eq!(listener["obfs"], "salamander");
        assert_eq!(listener["obfs-password"], "obfspw");
    }

    #[test]
    fn anytls_users_are_a_map() {
        let builder = MihomoConfigBuilder;
        let settings = json!({
            "tag": "anytls-in",
            "port": 9443,
            "clients": [{ "name": "carol", "password": "pw1" }],
        });
        let tls = json!({ "certFile": "/tmp/cert.pem", "keyFile": "/tmp/key.pem" });
        let listener = builder
            .build_inbound(ProtocolType::Anytls, &settings, Some(&tls))
            .unwrap();

        assert_eq!(listener["type"], "anytls");
        assert_eq!(listener["users"]["carol"], "pw1");
    }

    #[test]
    fn full_config_has_skeleton_and_skips_tlsless_hy2() {
        let builder = MihomoConfigBuilder;
        let reality = InboundConfig {
            tag: "vless-reality-in".into(),
            protocol: ProtocolType::VlessReality,
            listen: "0.0.0.0".into(),
            port: 443,
            settings: reality_settings(),
            tls: None,
            sniffing: None,
            core_version: None,
        };
        let hy2 = InboundConfig {
            tag: "hy2-in".into(),
            protocol: ProtocolType::Hysteria2,
            listen: "0.0.0.0".into(),
            port: 8444,
            settings: json!({ "tag": "hy2-in", "port": 8444, "clients": [] }),
            tls: None,
            sniffing: None,
            core_version: None,
        };

        let config = builder.build_full_config(&[reality, hy2]).unwrap();
        let listeners = config["listeners"].as_array().unwrap();
        assert_eq!(listeners.len(), 1);
        assert_eq!(listeners[0]["type"], "vless");
        assert_eq!(config["mode"], "rule");
        assert_eq!(config["rules"], json!(["MATCH,DIRECT"]));
        assert!(config["external-controller"].is_string());
    }

    #[test]
    fn full_config_serializes_to_valid_yaml() {
        let builder = MihomoConfigBuilder;
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
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed["listeners"][0]["type"].as_str().unwrap(), "vless");
    }
}
