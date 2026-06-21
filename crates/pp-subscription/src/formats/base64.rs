use base64::Engine;
use pp_common::{PanelError, PanelResult, ProtocolType};

use crate::generator::ProxyNode;

/// Generate base64-encoded subscription (traditional vmess/vless/trojan URLs).
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    let mut links = Vec::new();
    for node in nodes {
        let link = match node.protocol {
            ProtocolType::Vmess => generate_vmess_link(node)?,
            ProtocolType::VlessReality | ProtocolType::VlessVision | ProtocolType::VlessXhttp => {
                generate_vless_link(node)?
            }
            ProtocolType::Trojan => generate_trojan_link(node)?,
            ProtocolType::Shadowsocks2022 => generate_ss_link(node)?,
            _ => continue,
        };
        links.push(link);
    }

    let plain = links.join("\n");
    Ok(base64::engine::general_purpose::STANDARD.encode(plain))
}

fn generate_vmess_link(node: &ProxyNode) -> PanelResult<String> {
    let id = node
        .settings
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing vmess id".into()))?;

    let network = node
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut obj = serde_json::json!({
        "v": "2",
        "ps": node.name,
        "add": node.server,
        "port": node.port,
        "id": id,
        "aid": node.settings.get("alterId").and_then(|v| v.as_u64()).unwrap_or(0),
        "scy": node.settings.get("security").and_then(|v| v.as_str()).unwrap_or("auto"),
        "net": network,
        "type": node.settings.get("header_type").and_then(|v| v.as_str()).unwrap_or("none"),
        "host": "",
        "path": "",
        "tls": if node.tls.is_some() { "tls" } else { "" },
        "sni": node.tls.as_ref().and_then(|t| t.get("serverName")).and_then(|v| v.as_str()).unwrap_or(""),
    });

    match network {
        "ws" | "xhttp" | "httpupgrade" => {
            obj["path"] = node
                .settings
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("/")
                .into();
            obj["host"] = node
                .settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into();
        }
        "grpc" => {
            obj["path"] = node
                .settings
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into();
            obj["host"] = node
                .settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into();
            obj["type"] = "gun".into();
        }
        "kcp" | "mkcp" => {
            obj["path"] = node
                .settings
                .get("seed")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .into();
        }
        _ => {}
    }

    let b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&obj)?);
    Ok(format!("vmess://{}", b64))
}

fn generate_vless_link(node: &ProxyNode) -> PanelResult<String> {
    let id = node
        .settings
        .get("id")
        .and_then(|v| v.as_str())
        .or_else(|| {
            node.settings
                .get("clients")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("id"))
                .and_then(|v| v.as_str())
        })
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
                return Err(PanelError::Subscription("missing REALITY public_key".into()));
            }
            let server_name = node
                .settings
                .get("server_names")
                .and_then(|v| v.as_str())
                .or_else(|| node.settings.get("reality_server_names").and_then(|v| v.as_str()))
                .unwrap_or("")
                .split(',')
                .next()
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
            let short_id = node
                .settings
                .get("short_id")
                .and_then(|v| v.as_str())
                .or_else(|| node.settings.get("reality_short_id").and_then(|v| v.as_str()))
                .unwrap_or("")
                .split(',')
                .next()
                .map(|s| s.trim())
                .unwrap_or("")
                .to_string();
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

fn generate_trojan_link(node: &ProxyNode) -> PanelResult<String> {
    let password = node
        .settings
        .get("password")
        .and_then(|v| v.as_str())
        .or_else(|| {
            node.settings
                .get("clients")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("password"))
                .and_then(|v| v.as_str())
        })
        .ok_or_else(|| PanelError::Subscription("missing trojan password".into()))?;

    let network = node
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut params = vec![format!("type={}", network)];

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
        _ => {}
    }

    if let Some(tls) = &node.tls {
        if let Some(sni) = tls.get("serverName").and_then(|v| v.as_str()) {
            params.push(format!("sni={}", urlencoding::encode(sni)));
        }
    }

    let query = params.join("&");
    Ok(format!(
        "trojan://{}@{}:{}?{}#{}",
        password,
        node.server,
        node.port,
        query,
        urlencoding::encode(&node.name)
    ))
}

fn generate_ss_link(node: &ProxyNode) -> PanelResult<String> {
    let method = node
        .settings
        .get("method")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing ss method".into()))?;
    let password = node
        .settings
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing ss password".into()))?;

    let userinfo =
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", method, password));
    Ok(format!(
        "ss://{}@{}:{}#{}",
        userinfo,
        node.server,
        node.port,
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
                "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4",
                "flow": "xtls-rprx-vision",
                "public_key": "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",
                "server_names": "example.com,www.example.com",
                "short_id": "0123456789abcdef",
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
    fn vless_ws_link_contains_path_and_host() {
        let node = ProxyNode {
            name: "test-ws".into(),
            protocol: ProtocolType::VlessVision,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4",
                "network": "ws",
                "path": "/vless",
                "host": "cdn.example.com",
            }),
            tls: Some(json!({ "serverName": "cdn.example.com" })),
        };
        let link = generate_vless_link(&node).unwrap();
        assert!(link.starts_with("vless://"));
        assert!(link.contains("type=ws"));
        assert!(link.contains("path=%2Fvless"));
        assert!(link.contains("host=cdn.example.com"));
        assert!(link.contains("security=tls"));
    }

    #[test]
    fn trojan_grpc_link_contains_service_name() {
        let node = ProxyNode {
            name: "test-trojan-grpc".into(),
            protocol: ProtocolType::Trojan,
            server: "1.2.3.4".into(),
            port: 443,
            settings: json!({
                "password": "secret",
                "network": "grpc",
                "service_name": "trojan-grpc",
            }),
            tls: Some(json!({ "serverName": "example.com" })),
        };
        let link = generate_trojan_link(&node).unwrap();
        assert!(link.starts_with("trojan://"));
        assert!(link.contains("type=grpc"));
        assert!(link.contains("serviceName=trojan-grpc"));
    }
}
