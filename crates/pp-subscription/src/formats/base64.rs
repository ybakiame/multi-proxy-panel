use base64::Engine;
use pp_common::{PanelError, PanelResult, ProtocolType};

use crate::generator::ProxyNode;

/// Generate base64-encoded subscription (traditional proxy share URLs).
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    let mut links = Vec::new();
    for node in nodes {
        let link = match node.protocol {
            ProtocolType::VlessReality | ProtocolType::VlessXhttp => generate_vless_link(node)?,
            ProtocolType::Hysteria2 => generate_hysteria2_link(node)?,
            ProtocolType::Anytls => generate_anytls_link(node)?,
        };
        links.push(link);
    }

    let plain = links.join("\n");
    Ok(base64::engine::general_purpose::STANDARD.encode(plain))
}

fn generate_vless_link(node: &ProxyNode) -> PanelResult<String> {
    let id = pp_common::settings_helper::client_uuid(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing vless id".into()))?;

    let flow = node
        .settings
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Determine network from protocol type or settings
    let network = match node.protocol {
        ProtocolType::VlessXhttp => "xhttp",
        _ => node
            .settings
            .get("network")
            .and_then(|v| v.as_str())
            .unwrap_or("tcp"),
    };

    let mut params = vec![format!("type={}", network)];

    if !flow.is_empty() {
        params.push(format!("flow={}", flow));
    }

    // Transport-specific params
    match network {
        "ws" => {
            if let Some(path) = node.settings.get("path").and_then(|v| v.as_str()) {
                params.push(format!("path={}", urlencoding::encode(path)));
            }
            if let Some(host) = node.settings.get("host").and_then(|v| v.as_str()) {
                params.push(format!("host={}", urlencoding::encode(host)));
            }
        }
        "grpc" => {
            if let Some(service) = node.settings.get("service_name").and_then(|v| v.as_str()) {
                params.push(format!("serviceName={}", urlencoding::encode(service)));
            }
        }
        "xhttp" => {
            if let Some(path) = node.settings.get("xhttp_path").and_then(|v| v.as_str()) {
                params.push(format!("path={}", urlencoding::encode(path)));
            } else if let Some(path) = node.settings.get("path").and_then(|v| v.as_str()) {
                params.push(format!("path={}", urlencoding::encode(path)));
            }
            if let Some(host) = node.settings.get("xhttp_host").and_then(|v| v.as_str()) {
                params.push(format!("host={}", urlencoding::encode(host)));
            } else if let Some(host) = node.settings.get("host").and_then(|v| v.as_str()) {
                params.push(format!("host={}", urlencoding::encode(host)));
            }
            if let Some(mode) = node.settings.get("xhttp_mode").and_then(|v| v.as_str()) {
                params.push(format!("mode={}", mode));
            } else if let Some(mode) = node.settings.get("mode").and_then(|v| v.as_str()) {
                params.push(format!("mode={}", mode));
            }
        }
        "kcp" | "mkcp" => {
            if let Some(seed) = node.settings.get("seed").and_then(|v| v.as_str()) {
                params.push(format!("seed={}", urlencoding::encode(seed)));
            }
            if let Some(header_type) = node.settings.get("header_type").and_then(|v| v.as_str()) {
                params.push(format!("headerType={}", header_type));
            }
        }
        _ => {}
    }

    match node.protocol {
        ProtocolType::VlessReality => {
            let public_key = node
                .settings
                .get("public_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| PanelError::Subscription("missing REALITY public_key".into()))?;
            if public_key.is_empty() {
                return Err(PanelError::Subscription(
                    "missing REALITY public_key".into(),
                ));
            }
            let server_name =
                pp_common::settings_helper::first_server_name(&node.settings).unwrap_or_default();
            let short_id =
                pp_common::settings_helper::first_short_id(&node.settings).unwrap_or_default();
            let spider_x = node
                .settings
                .get("spider_x")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let fingerprint = node
                .settings
                .get("fingerprint")
                .and_then(|v| v.as_str())
                .unwrap_or("chrome");

            params.push("security=reality".to_string());
            if !server_name.is_empty() {
                params.push(format!("sni={}", urlencoding::encode(&server_name)));
            }
            params.push(format!("pbk={}", urlencoding::encode(public_key)));
            if !short_id.is_empty() {
                params.push(format!("sid={}", urlencoding::encode(&short_id)));
            }
            if !spider_x.is_empty() {
                params.push(format!("spx={}", urlencoding::encode(spider_x)));
            }
            params.push(format!("fp={}", fingerprint));
        }
        _ => {
            if node.tls.is_some() {
                params.push("security=tls".to_string());
                if let Some(sni) = tls_server_name(node) {
                    params.push(format!("sni={}", urlencoding::encode(&sni)));
                }
                if let Some(fp) = tls_fingerprint(node) {
                    params.push(format!("fp={}", fp));
                }
                if let Some(alpn) = tls_alpn(node) {
                    params.push(format!("alpn={}", urlencoding::encode(&alpn)));
                }
            }
        }
    }

    let query = params.join("&");
    Ok(format!(
        "vless://{}@{}:{}?{}#{}",
        id,
        node.server,
        node.port,
        query,
        encode_fragment(&node.name)
    ))
}

/// Encode a URL fragment so that spaces become `%20` instead of `+`.
/// Clients expect the display name after `#` to use percent-encoding, not
/// form-encoding.
fn encode_fragment(s: &str) -> String {
    urlencoding::encode(s).replace("+", "%20")
}

fn tls_server_name(node: &ProxyNode) -> Option<String> {
    node.tls
        .as_ref()
        .and_then(|t| t.get("serverName"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            node.settings
                .get("sni")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

fn tls_fingerprint(node: &ProxyNode) -> Option<String> {
    node.tls
        .as_ref()
        .and_then(|t| t.get("fingerprint"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            node.settings
                .get("fingerprint")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

fn tls_alpn(node: &ProxyNode) -> Option<String> {
    node.tls
        .as_ref()
        .and_then(|t| t.get("alpn"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            node.settings
                .get("alpn")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

fn generate_hysteria2_link(node: &ProxyNode) -> PanelResult<String> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing hysteria2 password".into()))?;

    let sni = tls_server_name(node).unwrap_or_else(|| node.server.clone());
    let skip_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(!pp_common::settings_helper::tls_has_real_certificate(
            node.tls.as_ref(),
        ));
    let insecure = skip_verify as u8;

    let mut params = vec![
        format!("sni={}", urlencoding::encode(&sni)),
        format!("insecure={}", insecure),
    ];

    if let Some(up) = node.settings.get("up_mbps").and_then(|v| v.as_u64()) {
        params.push(format!("upmbps={}", up));
    }
    if let Some(down) = node.settings.get("down_mbps").and_then(|v| v.as_u64()) {
        params.push(format!("downmbps={}", down));
    }
    if let Some(obfs_type) = node.settings.get("obfs_type").and_then(|v| v.as_str()) {
        if obfs_type != "none" {
            params.push(format!("obfs={}", obfs_type));
            if let Some(obfs_password) = node.settings.get("obfs_password").and_then(|v| v.as_str())
            {
                params.push(format!(
                    "obfs-password={}",
                    urlencoding::encode(obfs_password)
                ));
            }
        }
    }
    let alpn = tls_alpn(node).unwrap_or_else(|| "h3".to_string());
    params.push(format!("alpn={}", urlencoding::encode(&alpn)));

    let query = params.join("&");
    Ok(format!(
        "hysteria2://{}@{}:{}?{}#{}",
        urlencoding::encode(&password),
        node.server,
        node.port,
        query,
        encode_fragment(&node.name)
    ))
}

fn generate_anytls_link(node: &ProxyNode) -> PanelResult<String> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing anytls password".into()))?;

    let sni = tls_server_name(node).unwrap_or_else(|| node.server.clone());
    let skip_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(!pp_common::settings_helper::tls_has_real_certificate(
            node.tls.as_ref(),
        ));
    let insecure = skip_verify as u8;

    let mut params = vec![
        format!("sni={}", urlencoding::encode(&sni)),
        format!("insecure={}", insecure),
    ];

    if let Some(alpn) = tls_alpn(node) {
        params.push(format!("alpn={}", urlencoding::encode(&alpn)));
    }

    let query = params.join("&");
    Ok(format!(
        "anytls://{}@{}:{}?{}#{}",
        urlencoding::encode(&password),
        node.server,
        node.port,
        query,
        encode_fragment(&node.name)
    ))
}

mod urlencoding {
    pub fn encode(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::ProxyNode;
    use serde_json::json;

    fn reality_node() -> ProxyNode {
        ProxyNode {
            name: "test-reality".into(),
            protocol: ProtocolType::VlessReality,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "id": "",
                "clients": [{"id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4"}],
                "flow": "xtls-rprx-vision",
                "public_key": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                "server_names": ["example.com", "www.example.com"],
                "short_id": ["0123456789abcdef"],
                "fingerprint": "chrome"
            }),
            tls: None,
        }
    }

    #[test]
    fn vless_reality_link_contains_required_params() {
        let link = generate_vless_link(&reality_node()).unwrap();
        assert!(link.starts_with("vless://"));
        assert!(link.contains("security=reality"));
        assert!(link.contains("pbk=BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"));
        assert!(link.contains("sid=0123456789abcdef"));
        assert!(link.contains("sni=example.com"));
        assert!(link.contains("flow=xtls-rprx-vision"));
        assert!(link.contains("fp=chrome"));
    }

    #[test]
    fn hysteria2_link_contains_required_params() {
        let node = ProxyNode {
            name: "test-hy2".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "password": "",
                "clients": [{"password": "hy2-secret"}],
                "up_mbps": 200,
                "down_mbps": 100,
                "obfs_type": "salamander",
                "obfs_password": "obfs-secret",
                "skip_cert_verify": false,
            }),
            tls: Some(json!({ "serverName": "hy2.example.com" })),
        };
        let link = generate_hysteria2_link(&node).unwrap();
        assert!(link.starts_with("hysteria2://"));
        assert!(link.contains("hy2-secret@1.2.3.4:8443"));
        assert!(link.contains("sni=hy2.example.com"));
        assert!(link.contains("insecure=0"));
        assert!(link.contains("upmbps=200"));
        assert!(link.contains("downmbps=100"));
        assert!(link.contains("obfs=salamander"));
        assert!(link.contains("obfs-password=obfs-secret"));
        assert!(link.contains("alpn=h3"));
    }

    #[test]
    fn hysteria2_default_alpn_is_h3() {
        let node = ProxyNode {
            name: "hy2 default".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "clients": [{"password": "hy2-secret"}],
            }),
            tls: None,
        };
        let link = generate_hysteria2_link(&node).unwrap();
        assert!(link.contains("alpn=h3"));
        assert!(link.ends_with("#hy2%20default"));
    }

    #[test]
    fn anytls_link_contains_required_params() {
        let node = ProxyNode {
            name: "test-anytls".into(),
            protocol: ProtocolType::Anytls,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "password": "",
                "clients": [{"password": "anytls-secret"}],
                "skip_cert_verify": true,
            }),
            tls: Some(json!({ "serverName": "anytls.example.com" })),
        };
        let link = generate_anytls_link(&node).unwrap();
        assert!(link.starts_with("anytls://"));
        assert!(link.contains("anytls-secret@1.2.3.4:443"));
        assert!(link.contains("sni=anytls.example.com"));
        assert!(link.contains("insecure=1"));
    }

    #[test]
    fn anytls_link_skips_verify_without_real_certificate() {
        let node = ProxyNode {
            name: "test-anytls".into(),
            protocol: ProtocolType::Anytls,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "password": "",
                "clients": [{"password": "anytls-secret"}],
            }),
            tls: Some(json!({ "serverName": "anytls.example.com" })),
        };
        let link = generate_anytls_link(&node).unwrap();
        assert!(link.contains("insecure=1"));
    }

    #[test]
    fn anytls_link_verifies_when_acme_domain_present() {
        let node = ProxyNode {
            name: "test-anytls".into(),
            protocol: ProtocolType::Anytls,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "password": "",
                "clients": [{"password": "anytls-secret"}],
            }),
            tls: Some(json!({ "domain": "anytls.example.com" })),
        };
        let link = generate_anytls_link(&node).unwrap();
        assert!(link.contains("insecure=0"));
    }

    #[test]
    fn base64_subscription_includes_all_protocols() {
        let vless = reality_node();
        let hy2 = ProxyNode {
            name: "test-hy2".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "clients": [{"password": "hy2-secret"}],
            }),
            tls: None,
        };
        let anytls = ProxyNode {
            name: "test-anytls".into(),
            protocol: ProtocolType::Anytls,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "clients": [{"password": "anytls-secret"}],
            }),
            tls: None,
        };

        let encoded = generate(&[vless, hy2, anytls]).unwrap();
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .unwrap();
        let plain = String::from_utf8(decoded).unwrap();
        assert!(plain.contains("vless://"));
        assert!(plain.contains("hysteria2://"));
        assert!(plain.contains("anytls://"));
    }
}
