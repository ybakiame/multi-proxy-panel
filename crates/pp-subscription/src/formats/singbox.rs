use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::Value;

use crate::generator::ProxyNode;

/// Protocols supported by the sing-box subscription generator.
fn is_supported(protocol: ProtocolType) -> bool {
    matches!(
        protocol,
        ProtocolType::VlessReality | ProtocolType::Hysteria2 | ProtocolType::Anytls
    )
}

/// Generate sing-box JSON subscription with outbounds array.
/// `base_config` is raw JSON template text. Supported placeholders:
///   - `<OUTBOUND_REPLACE>`  -> JSON array of generated outbounds
///   - `<NODE_REPLACE>`     -> JSON array of node names
pub fn generate(nodes: &[ProxyNode], base_config: Option<&str>) -> PanelResult<String> {
    let supported: Vec<&ProxyNode> = nodes.iter().filter(|n| is_supported(n.protocol)).collect();

    let outbounds: Vec<_> = supported
        .iter()
        .map(|n| build_outbound(n))
        .collect::<Result<Vec<_>, _>>()?;

    let node_names: Vec<String> = supported.iter().map(|n| n.name.clone()).collect();

    if let Some(base) = base_config {
        if base.contains("<OUTBOUND_REPLACE>") || base.contains("<NODE_REPLACE>") {
            let rendered = render_template(base.to_string(), &outbounds, &node_names)?;
            return Ok(serde_json::to_string_pretty(&rendered)?);
        }

        let mut config: Value = serde_json::from_str(base).map_err(|e| {
            PanelError::Subscription(format!("failed to parse sing-box template json: {e}"))
        })?;
        config["outbounds"] = serde_json::Value::Array(outbounds);
        return Ok(serde_json::to_string_pretty(&config)?);
    }

    let mut config = serde_json::json!({ "outbounds": [] });
    config["outbounds"] = serde_json::Value::Array(outbounds);
    Ok(serde_json::to_string_pretty(&config)?)
}

fn render_template(
    base_str: String,
    outbounds: &[Value],
    node_names: &[String],
) -> PanelResult<Value> {
    let outbounds_json = serde_json::to_string(&outbounds)?;
    let names_json = serde_json::to_string(&node_names)?;

    let rendered = base_str
        .replace("\"<OUTBOUND_REPLACE>\"", &outbounds_json)
        .replace("\"<NODE_REPLACE>\"", &names_json);

    serde_json::from_str(&rendered).map_err(|e| {
        PanelError::Subscription(format!("failed to render subscription template: {e}"))
    })
}

fn build_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    match node.protocol {
        ProtocolType::VlessReality => build_vless_reality_outbound(node),
        ProtocolType::Hysteria2 => build_hysteria2_outbound(node),
        ProtocolType::Anytls => build_anytls_outbound(node),
        ProtocolType::VlessXhttp => Err(PanelError::Subscription(
            "sing-box does not support the vless xhttp transport".into(),
        )),
    }
}

fn build_vless_reality_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let uuid = pp_common::settings_helper::client_uuid(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing vless uuid".into()))?;

    let flow = node
        .settings
        .get("flow")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let network = node
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut outbound = serde_json::json!({
        "type": "vless",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "uuid": uuid,
        "packet_encoding": "xudp",
    });

    if !flow.is_empty() {
        outbound["flow"] = serde_json::json!(flow);
    }

    if network != "tcp" {
        outbound["transport"] = serde_json::json!({
            "type": network,
        });
        if let Some(path) = node.settings.get("path").and_then(|v| v.as_str()) {
            outbound["transport"]["path"] = serde_json::json!(path);
        }
        if let Some(host) = node.settings.get("host").and_then(|v| v.as_str()) {
            outbound["transport"]["host"] = serde_json::json!(host);
        }
    }

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
    let short_id = pp_common::settings_helper::first_short_id(&node.settings).unwrap_or_default();
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
        "short_id": short_id,
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

    Ok(outbound)
}

fn build_hysteria2_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing hysteria2 password".into()))?;

    let server_name = node
        .tls
        .as_ref()
        .and_then(|t| t.get("serverName"))
        .and_then(|v| v.as_str())
        .or_else(|| node.settings.get("sni").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let skip_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(!pp_common::settings_helper::tls_has_real_certificate(
            node.tls.as_ref(),
        ));

    let mut outbound = serde_json::json!({
        "type": "hysteria2",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "password": password,
        "tls": {
            "enabled": true,
            "server_name": server_name,
            "insecure": skip_verify,
        },
    });

    if let Some(up) = node.settings.get("up_mbps").and_then(|v| v.as_u64()) {
        outbound["up_mbps"] = serde_json::json!(up);
    }
    if let Some(down) = node.settings.get("down_mbps").and_then(|v| v.as_u64()) {
        outbound["down_mbps"] = serde_json::json!(down);
    }
    if let Some(obfs_type) = node.settings.get("obfs_type").and_then(|v| v.as_str()) {
        if obfs_type != "none" {
            if let Some(obfs_password) = node.settings.get("obfs_password").and_then(|v| v.as_str())
            {
                outbound["obfs"] = serde_json::json!({
                    "type": obfs_type,
                    "password": obfs_password,
                });
            }
        }
    }

    Ok(outbound)
}

fn build_anytls_outbound(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing anytls password".into()))?;

    let server_name = node
        .tls
        .as_ref()
        .and_then(|t| t.get("serverName"))
        .and_then(|v| v.as_str())
        .or_else(|| node.settings.get("sni").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let skip_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(!pp_common::settings_helper::tls_has_real_certificate(
            node.tls.as_ref(),
        ));

    Ok(serde_json::json!({
        "type": "anytls",
        "tag": node.name,
        "server": node.server,
        "server_port": node.port,
        "password": password,
        "tls": {
            "enabled": true,
            "server_name": server_name,
            "insecure": skip_verify,
        },
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
    fn vless_reality_outbound_has_reality_tls() {
        let outbound = build_vless_reality_outbound(&reality_node()).unwrap();
        assert_eq!(outbound["type"], "vless");
        assert_eq!(outbound["server"], "1.2.3.4");
        assert_eq!(outbound["server_port"], 443);
        assert_eq!(outbound["flow"], "xtls-rprx-vision");
        assert_eq!(outbound["packet_encoding"], "xudp");
        assert_eq!(outbound["tls"]["enabled"], true);
        assert_eq!(outbound["tls"]["server_name"], "example.com");
        assert_eq!(
            outbound["tls"]["reality"]["public_key"],
            "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB"
        );
        assert_eq!(outbound["tls"]["reality"]["short_id"], "0123456789abcdef");
        assert_eq!(outbound["tls"]["utls"]["fingerprint"], "chrome");
    }

    #[test]
    fn hysteria2_outbound_insecure_when_no_real_certificate() {
        let node = ProxyNode {
            name: "test-hy2".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "clients": [{"password": "hy2-secret"}],
            }),
            tls: Some(json!({ "serverName": "hy2.example.com" })),
        };
        let outbound = build_hysteria2_outbound(&node).unwrap();
        assert_eq!(outbound["tls"]["insecure"], true);
    }

    #[test]
    fn hysteria2_outbound_verifies_when_acme_domain_present() {
        let node = ProxyNode {
            name: "test-hy2".into(),
            protocol: ProtocolType::Hysteria2,
            server: "1.2.3.4".into(),
            port: 8443,
            settings: json!({
                "clients": [{"password": "hy2-secret"}],
            }),
            tls: Some(json!({ "domain": "hy2.example.com" })),
        };
        let outbound = build_hysteria2_outbound(&node).unwrap();
        assert_eq!(outbound["tls"]["insecure"], false);
    }

    #[test]
    fn vless_reality_outbound_requires_public_key() {
        let mut node = reality_node();
        node.settings["public_key"] = json!("");
        let err = build_vless_reality_outbound(&node).unwrap_err();
        assert!(err.to_string().contains("public_key"));
    }
}
