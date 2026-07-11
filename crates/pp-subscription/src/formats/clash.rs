use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::Value;
use serde_json::json;

use crate::generator::ProxyNode;

/// Generate Clash Meta / Mihomo YAML subscription.
/// `base_config` is raw YAML template text. Supported placeholders:
///   - `<PROXY_REPLACE>`  -> YAML list of generated proxies
///   - `<NODE_REPLACE>`   -> YAML list of proxy names
///
/// Comments and formatting in the template are preserved.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&str>) -> PanelResult<String> {
    let mut proxies = Vec::new();
    let mut proxy_names = Vec::new();

    for node in nodes {
        proxy_names.push(node.name.clone());
        proxies.push(build_proxy(node)?);
    }

    if let Some(base) = base_config {
        let trimmed = base.trim();
        if trimmed.contains("<PROXY_REPLACE>") || trimmed.contains("<NODE_REPLACE>") {
            return render_yaml_template(base, &proxies, &proxy_names);
        }

        // No placeholders: parse as YAML, merge proxies/proxy-groups, serialize back.
        let mut base_value: Value = serde_yaml::from_str(base).map_err(|e| {
            PanelError::Subscription(format!("failed to parse clash template yaml: {e}"))
        })?;

        if base_value.get("proxies").is_none() {
            base_value["proxies"] = Value::Array(Vec::new());
        }
        if let Some(base_proxies) = base_value["proxies"].as_array_mut() {
            for proxy in &proxies {
                base_proxies.push(proxy.clone());
            }
        }

        if base_value.get("proxy-groups").is_none() {
            base_value["proxy-groups"] = default_proxy_groups(&proxy_names);
        }

        return serde_yaml::to_string(&base_value)
            .map_err(|e| PanelError::Subscription(format!("failed to serialize clash yaml: {e}")));
    }

    let output = serde_json::json!({
        "proxies": proxies,
        "proxy-groups": default_proxy_groups(&proxy_names),
    });

    serde_yaml::to_string(&output)
        .map_err(|e| PanelError::Subscription(format!("failed to serialize clash yaml: {e}")))
}

fn default_proxy_groups(proxy_names: &[String]) -> Value {
    serde_json::json!([
        {
            "name": "Proxy",
            "type": "select",
            "proxies": proxy_names
        }
    ])
}

/// Render a YAML template preserving comments and formatting.
fn render_yaml_template(
    template: &str,
    proxies: &[Value],
    proxy_names: &[String],
) -> PanelResult<String> {
    let proxies_yaml = serde_yaml::to_string(proxies).map_err(|e| {
        PanelError::Subscription(format!("failed to serialize proxies to yaml: {e}"))
    })?;
    let names_yaml = serde_yaml::to_string(proxy_names).map_err(|e| {
        PanelError::Subscription(format!("failed to serialize proxy names to yaml: {e}"))
    })?;

    let mut output = template.to_string();
    output = replace_yaml_placeholder(&output, "<PROXY_REPLACE>", &proxies_yaml)?;
    output = replace_yaml_placeholder(&output, "<NODE_REPLACE>", &names_yaml)?;

    Ok(output)
}

/// Replace a placeholder in a YAML template with an indented YAML block.
fn replace_yaml_placeholder(
    template: &str,
    placeholder: &str,
    replacement_yaml: &str,
) -> PanelResult<String> {
    let pos = template
        .find(placeholder)
        .ok_or_else(|| PanelError::Subscription(format!("placeholder {placeholder} not found")))?;

    // Find the start of the line containing the placeholder.
    let line_start = template[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let indent = &template[line_start..pos];
    let indent_spaces = indent.len() - indent.trim_start().len();
    let indent_str = " ".repeat(indent_spaces);

    // The leading spaces of the template line provide indentation for the first
    // line of the replacement block; subsequent lines are indented to match.
    let trimmed = replacement_yaml.trim_start_matches('\n').trim_end();
    let mut lines = trimmed.lines();
    let first = lines.next().unwrap_or("");
    let mut result = first.to_string();
    for line in lines {
        result.push('\n');
        if !line.is_empty() {
            result.push_str(&indent_str);
        }
        result.push_str(line);
    }

    Ok(template.replace(placeholder, &result))
}

fn build_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    match node.protocol {
        ProtocolType::VlessReality | ProtocolType::VlessVision | ProtocolType::VlessXhttp => {
            build_vless_proxy(node)
        }
        ProtocolType::Vmess => build_vmess_proxy(node),
        ProtocolType::Trojan => build_trojan_proxy(node),
        ProtocolType::Shadowsocks2022 => build_shadowsocks_proxy(node),
        ProtocolType::Hysteria2 => build_hysteria2_proxy(node),
        _ => Err(PanelError::Subscription(format!(
            "protocol {:?} not supported in clash subscription",
            node.protocol
        ))),
    }
}

fn build_vless_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let uuid = pp_common::settings_helper::client_uuid(&node.settings)
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

    let mut proxy = serde_json::json!({
        "name": node.name,
        "type": "vless",
        "server": node.server,
        "port": node.port,
        "uuid": uuid,
        "network": network,
        "udp": true,
    });

    if !flow.is_empty() {
        proxy["flow"] = json!(flow);
    }

    match node.protocol {
        ProtocolType::VlessReality => {
            let public_key = node
                .settings
                .get("public_key")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let short_id = node
                .settings
                .get("short_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let server_name = node
                .settings
                .get("server_name")
                .or_else(|| node.settings.get("sni"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let fingerprint = node
                .settings
                .get("client_fingerprint")
                .or_else(|| node.settings.get("fingerprint"))
                .and_then(|v| v.as_str())
                .unwrap_or("chrome");

            proxy["tls"] = json!(true);
            proxy["servername"] = json!(server_name);
            proxy["client-fingerprint"] = json!(fingerprint);
            proxy["reality-opts"] = json!({
                "enabled": true,
                "public-key": public_key,
                "short-id": short_id,
            });
        }
        ProtocolType::VlessVision => {
            proxy["tls"] = json!(true);
        }
        ProtocolType::VlessXhttp => {
            proxy["tls"] = json!(true);
            let host = node
                .settings
                .get("host")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let path = node
                .settings
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("/");
            proxy["plugin-opts"] = json!({
                "mode": "xhttp",
                "host": host,
                "path": path,
            });
        }
        _ => {}
    }

    apply_transport_settings(&mut proxy, network, &node.settings, node.tls.as_ref())?;

    Ok(proxy)
}

fn build_vmess_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let uuid = pp_common::settings_helper::client_uuid(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing vmess uuid".into()))?;

    let network = node
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut proxy = serde_json::json!({
        "name": node.name,
        "type": "vmess",
        "server": node.server,
        "port": node.port,
        "uuid": uuid,
        "alterId": 0,
        "cipher": "auto",
        "network": network,
        "udp": true,
    });

    apply_transport_settings(&mut proxy, network, &node.settings, node.tls.as_ref())?;

    Ok(proxy)
}

fn build_trojan_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing trojan password".into()))?;

    let network = node
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp");

    let mut proxy = serde_json::json!({
        "name": node.name,
        "type": "trojan",
        "server": node.server,
        "port": node.port,
        "password": password,
        "network": network,
        "udp": true,
    });

    apply_transport_settings(&mut proxy, network, &node.settings, node.tls.as_ref())?;

    Ok(proxy)
}

fn build_shadowsocks_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing shadowsocks password".into()))?;

    let method = node
        .settings
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("2022-blake3-aes-128-gcm");

    Ok(serde_json::json!({
        "name": node.name,
        "type": "ss",
        "server": node.server,
        "port": node.port,
        "password": password,
        "cipher": method,
        "udp": true,
    }))
}

fn build_hysteria2_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = pp_common::settings_helper::client_password(&node.settings)
        .ok_or_else(|| PanelError::Subscription("missing hysteria2 password".into()))?;

    let sni = node
        .settings
        .get("sni")
        .and_then(|v| v.as_str())
        .unwrap_or(&node.server);
    let skip_cert_verify = node
        .settings
        .get("skip_cert_verify")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    Ok(serde_json::json!({
        "name": node.name,
        "type": "hysteria2",
        "server": node.server,
        "port": node.port,
        "password": password,
        "sni": sni,
        "skip-cert-verify": skip_cert_verify,
        "up": node.settings.get("up").and_then(|v| v.as_u64()).unwrap_or(100),
        "down": node.settings.get("down").and_then(|v| v.as_u64()).unwrap_or(100),
        "udp": true,
    }))
}

fn apply_transport_settings(
    proxy: &mut Value,
    network: &str,
    settings: &Value,
    tls: Option<&Value>,
) -> Result<(), PanelError> {
    match network {
        "ws" => {
            let path = settings.get("path").and_then(|v| v.as_str()).unwrap_or("/");
            let host = settings.get("host").and_then(|v| v.as_str()).unwrap_or("");
            proxy["ws-opts"] = json!({
                "path": path,
                "headers": {
                    "Host": host
                }
            });
        }
        "grpc" => {
            let service_name = settings
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            proxy["grpc-opts"] = json!({
                "grpc-service-name": service_name,
            });
        }
        "tcp" => {
            if let Some(tls_cfg) = tls {
                proxy["tls"] = json!(true);
                if let Some(sni) = tls_cfg.get("serverName").and_then(|v| v.as_str()) {
                    proxy["sni"] = json!(sni);
                }
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reality_node() -> ProxyNode {
        ProxyNode {
            name: "test-reality".to_string(),
            protocol: ProtocolType::VlessReality,
            server: "example.com".to_string(),
            port: 443,
            settings: serde_json::json!({
                "id": "a4a4a4a4-a4a4-a4a4-a4a4-a4a4a4a4a4a4",
                "public_key": "pbk",
                "short_id": "sid",
                "server_name": "www.example.com",
                "flow": "xtls-rprx-vision",
            }),
            tls: None,
        }
    }

    #[test]
    fn clash_default_output_is_yaml() {
        let out = generate(&[reality_node()], None).unwrap();
        assert!(out.starts_with("proxies:"));
        assert!(out.contains("type: vless"));
        assert!(out.contains("reality-opts:"));
    }

    #[test]
    fn clash_template_replaces_placeholders() {
        let base = r#"
port: 7890
# This is a comment
proxies:
  <PROXY_REPLACE>
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      <NODE_REPLACE>
"#;
        let out = generate(&[reality_node()], Some(base)).unwrap();
        assert!(out.contains("port: 7890"));
        assert!(out.contains("# This is a comment"));
        assert!(out.contains("type: vless"));
        assert!(out.contains("name: test-reality"));
        assert!(out.contains("- test-reality"));

        // Must be valid YAML with proxies under the correct key.
        let parsed: Value = serde_yaml::from_str(&out).unwrap();
        let proxies = parsed["proxies"]
            .as_array()
            .expect("proxies should be a list");
        assert_eq!(proxies.len(), 1);
        assert_eq!(proxies[0]["name"], "test-reality");
        let groups = parsed["proxy-groups"]
            .as_array()
            .expect("proxy-groups should be a list");
        let proxy_names: Vec<_> = groups[0]["proxies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(proxy_names, vec!["test-reality"]);
    }
}
