use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::Value;

use crate::generator::ProxyNode;

/// Generate sing-box JSON subscription with outbounds array.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&Value>) -> PanelResult<String> {
    let outbounds: Vec<_> = nodes
        .iter()
        .map(build_outbound)
        .collect::<Result<Vec<_>, _>>()?;

    let mut config = if let Some(base) = base_config {
        base.clone()
    } else {
        serde_json::json!({ "outbounds": [] })
    };

    config["outbounds"] = serde_json::Value::Array(outbounds);
    Ok(serde_json::to_string_pretty(&config)?)
}

fn build_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    match node.protocol {
        ProtocolType::VlessReality | ProtocolType::VlessVision | ProtocolType::VlessXhttp => {
            build_vless_outbound(node)
        }
        ProtocolType::Vmess => build_vmess_outbound(node),
        ProtocolType::Trojan => build_trojan_outbound(node),
        ProtocolType::Shadowsocks2022 => build_shadowsocks_outbound(node),
        _ => Err(PanelError::Subscription(format!(
            "protocol {:?} not supported in sing-box subscription",
            node.protocol
        ))),
    }
}

fn build_vless_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let uuid = node
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
        .ok_or_else(|| PanelError::Subscription("missing vless uuid".into()))?;

    let flow = node
        .settings
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let network = match node.protocol {
        ProtocolType::VlessXhttp => "xhttp",
        _ => node
            .settings
            .get("network")
            .and_then(|v| v.as_str())
            .unwrap_or("tcp"),
    };

    let mut outbound = serde_json::json!({
        "type": "vless",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "uuid": uuid,
    });

    if !flow.is_empty() {
        outbound["flow"] = serde_json::json!(flow);
    }

    if network != "tcp" {
        outbound["transport"] = serde_json::json!({
            "type": network,
        });
        if network == "xhttp" {
            if let Some(path) = node.settings.get("xhttp_path").and_then(|v| v.as_str()) {
                outbound["transport"]["path"] = serde_json::json!(path);
            } else if let Some(path) = node.settings.get("path").and_then(|v| v.as_str()) {
                outbound["transport"]["path"] = serde_json::json!(path);
            }
            if let Some(host) = node.settings.get("xhttp_host").and_then(|v| v.as_str()) {
                outbound["transport"]["host"] = serde_json::json!(host);
            } else if let Some(host) = node.settings.get("host").and_then(|v| v.as_str()) {
                outbound["transport"]["host"] = serde_json::json!(host);
            }
        }
    }

    if node.protocol == ProtocolType::VlessReality {
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
        let short_ids: Vec<String> = node
            .settings
            .get("short_id")
            .and_then(|v| v.as_str())
            .or_else(|| node.settings.get("reality_short_id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
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

        let mut reality = serde_json::json!({
            "enabled": true,
            "public_key": public_key,
            "short_id": if short_ids.is_empty() { serde_json::json!([""]) } else { serde_json::json!(short_ids) },
        });
        if !spider_x.is_empty() {
            reality["spider_x"] = serde_json::json!(spider_x);
        }

        outbound["tls"] = serde_json::json!({
            "enabled": true,
            "server_name": server_name,
            "utls": {
                "enabled": true,
                "fingerprint": fingerprint,
            },
            "reality": reality,
        });
    } else if let Some(tls) = &node.tls {
        let mut tls_obj = serde_json::json!({
            "enabled": true,
            "server_name": tls.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
        });
        if network != "tcp" {
            tls_obj["utls"] = serde_json::json!({
                "enabled": true,
                "fingerprint": node.settings.get("fingerprint").and_then(|v| v.as_str()).unwrap_or("chrome"),
            });
        }
        outbound["tls"] = tls_obj;
    }

    Ok(outbound)
}

fn build_vmess_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let id = node
        .settings
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing vmess id".into()))?;
    Ok(serde_json::json!({
        "type": "vmess",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "uuid": id,
        "security": node.settings.get("security").and_then(|v| v.as_str()).unwrap_or("auto"),
        "alter_id": node.settings.get("alterId").and_then(|v| v.as_u64()).unwrap_or(0),
    }))
}

fn build_trojan_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = node
        .settings
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing trojan password".into()))?;
    let mut outbound = serde_json::json!({
        "type": "trojan",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "password": password,
    });
    if let Some(tls) = &node.tls {
        outbound["tls"] = serde_json::json!({
            "enabled": true,
            "server_name": tls.get("serverName").and_then(|v| v.as_str()).unwrap_or(""),
        });
    }
    Ok(outbound)
}

fn build_shadowsocks_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let method = node
        .settings
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("2022-blake3-aes-128-gcm");
    let password = node
        .settings
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing shadowsocks password".into()))?;
    Ok(serde_json::json!({
        "type": "shadowsocks",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "method": method,
        "password": password,
    }))
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
    fn vless_reality_outbound_has_reality_tls() {
        let outbound = build_vless_outbound(&reality_node()).unwrap();
        assert_eq!(outbound["type"], "vless");
        assert_eq!(outbound["server"], "1.2.3.4");
        assert_eq!(outbound["server_port"], 443);
        assert_eq!(outbound["flow"], "xtls-rprx-vision");
        assert_eq!(outbound["tls"]["enabled"], true);
        assert_eq!(outbound["tls"]["server_name"], "example.com");
        assert_eq!(
            outbound["tls"]["reality"]["public_key"],
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
        );
        assert_eq!(
            outbound["tls"]["utls"]["fingerprint"],
            "chrome"
        );
    }

    #[test]
    fn vless_reality_outbound_requires_public_key() {
        let mut node = reality_node();
        node.settings["public_key"] = json!("");
        let err = build_vless_outbound(&node).unwrap_err();
        assert!(err.to_string().contains("public_key"));
    }
}
