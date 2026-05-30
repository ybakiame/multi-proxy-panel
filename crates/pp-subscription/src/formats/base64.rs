use base64::Engine;
use pp_common::{PanelError, PanelResult, ProtocolType};

use crate::generator::ProxyNode;

/// Generate base64-encoded subscription (traditional vmess/vless/trojan URLs).
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    let mut links = Vec::new();
    for node in nodes {
        let link = match node.protocol {
            ProtocolType::Vmess => generate_vmess_link(node)?,
            ProtocolType::VlessReality
            | ProtocolType::VlessVision
            | ProtocolType::VlessXhttp => generate_vless_link(node)?,
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

    let obj = serde_json::json!({
        "v": "2",
        "ps": node.name,
        "add": node.server,
        "port": node.port,
        "id": id,
        "aid": node.settings.get("alterId").and_then(|v| v.as_u64()).unwrap_or(0),
        "scy": node.settings.get("security").and_then(|v| v.as_str()).unwrap_or("auto"),
        "net": node.settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp"),
        "type": node.settings.get("type").and_then(|v| v.as_str()).unwrap_or("none"),
        "host": node.settings.get("host").and_then(|v| v.as_str()).unwrap_or(""),
        "path": node.settings.get("path").and_then(|v| v.as_str()).unwrap_or(""),
        "tls": if node.tls.is_some() { "tls" } else { "" },
        "sni": node.tls.as_ref().and_then(|t| t.get("serverName")).and_then(|v| v.as_str()).unwrap_or(""),
    });

    let b64 = base64::engine::general_purpose::STANDARD.encode(serde_json::to_string(&obj)?);
    Ok(format!("vmess://{}", b64))
}

fn generate_vless_link(node: &ProxyNode) -> PanelResult<String> {
    let id = node
        .settings
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing vless id".into()))?;

    let flow = node
        .settings
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Determine network from protocol type or settings
    let network = match node.protocol {
        ProtocolType::VlessXhttp => "xhttp",
        _ => node.settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp"),
    };

    let mut params = vec![format!("type={}", network)];

    if !flow.is_empty() {
        params.push(format!("flow={}", flow));
    }

    // XHTTP specific params
    if network == "xhttp" {
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

    if let Some(tls) = &node.tls {
        params.push("security=tls".to_string());
        if let Some(sni) = tls.get("serverName").and_then(|v| v.as_str()) {
            params.push(format!("sni={}", urlencoding::encode(sni)));
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
        .ok_or_else(|| PanelError::Subscription("missing trojan password".into()))?;

    let mut params = vec![
        format!("type={}", node.settings.get("network").and_then(|v| v.as_str()).unwrap_or("tcp")),
    ];

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

    let userinfo = base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", method, password));
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
