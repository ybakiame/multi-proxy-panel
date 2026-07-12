use base64::Engine;
use pp_common::{PanelError, PanelResult, ProtocolType};

use crate::generator::ProxyNode;

/// Generate base64-encoded subscription (traditional vless URLs).
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    let mut links = Vec::new();
    for node in nodes {
        let link = match node.protocol {
            ProtocolType::VlessReality | ProtocolType::VlessXhttp => generate_vless_link(node)?,
            _ => continue,
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
            if let Some(tls) = &node.tls {
                params.push("security=tls".to_string());
                if let Some(sni) = tls.get("serverName").and_then(|v| v.as_str()) {
                    params.push(format!("sni={}", urlencoding::encode(sni)));
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
        urlencoding::encode(&node.name)
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
    fn vless_xhttp_link_contains_path_and_host() {
        let node = ProxyNode {
            name: "test-xhttp".into(),
            protocol: ProtocolType::VlessXhttp,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4",
                "xhttp_path": "/xhttp",
                "xhttp_host": "cdn.example.com",
                "xhttp_mode": "auto",
            }),
            tls: Some(json!({ "serverName": "cdn.example.com" })),
        };
        let link = generate_vless_link(&node).unwrap();
        assert!(link.starts_with("vless://"));
        assert!(link.contains("type=xhttp"));
        assert!(link.contains("path=%2Fxhttp"));
        assert!(link.contains("host=cdn.example.com"));
        assert!(link.contains("security=tls"));
    }
}
