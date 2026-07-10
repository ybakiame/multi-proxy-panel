use pp_common::{PanelError, PanelResult, ProtocolType};
use serde_json::Value;

use crate::generator::ProxyNode;

/// Generate Clash Meta / Mihomo YAML subscription.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&Value>) -> PanelResult<String> {
    let mut proxies = Vec::new();
    let mut proxy_names = Vec::new();

    for node in nodes {
        proxy_names.push(node.name.clone());
        proxies.push(build_proxy(node)?);
    }

    if let Some(base) = base_config {
        let base_str = serde_json::to_string(base)?;
        if base_str.contains("\"<PROXY_REPLACE>\"") || base_str.contains("\"<NODE_REPLACE>\"") {
            let rendered = render_template(base_str, &proxies, &proxy_names)?;
            return Ok(serde_yaml::to_string(&rendered)?);
        }

        let mut output = default_output(&proxies, &proxy_names);
        if let Some(base_proxies) = base.get("proxies").and_then(|v| v.as_array()) {
            let mut merged = base_proxies.clone();
            merged.extend(proxies.clone());
            output["proxies"] = serde_json::Value::Array(merged);
        }
        if let Some(base_groups) = base.get("proxy-groups").and_then(|v| v.as_array()) {
            let mut merged = base_groups.clone();
            merged.extend(
                output["proxy-groups"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default(),
            );
            output["proxy-groups"] = serde_json::Value::Array(merged);
        }
        return Ok(serde_yaml::to_string(&output)?);
    }

    Ok(serde_yaml::to_string(&default_output(
        &proxies,
        &proxy_names,
    ))?)
}

fn default_output(proxies: &[Value], proxy_names: &[String]) -> Value {
    serde_json::json!({
        "proxies": proxies,
        "proxy-groups": [
            {
                "name": "Proxy",
                "type": "select",
                "proxies": proxy_names
            }
        ]
    })
}

fn render_template(
    base_str: String,
    proxies: &[Value],
    proxy_names: &[String],
) -> PanelResult<Value> {
    let proxies_json = serde_json::to_string(&proxies)?;
    let names_json = serde_json::to_string(&proxy_names)?;

    let rendered = base_str
        .replace("\"<PROXY_REPLACE>\"", &proxies_json)
        .replace("\"<NODE_REPLACE>\"", &names_json);

    serde_json::from_str(&rendered).map_err(|e| {
        PanelError::Subscription(format!("failed to render subscription template: {e}"))
    })
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

    let mut proxy = serde_json::json!({
        "name": node.name,
        "type": "vless",
        "server": node.server,
        "port": node.port,
        "uuid": uuid,
        "network": network,
    });

    if !flow.is_empty() {
        proxy["flow"] = serde_json::json!(flow);
    }

    if network == "xhttp" {
        if let Some(path) = node.settings.get("xhttp_path").and_then(|v| v.as_str()) {
            proxy["path"] = serde_json::json!(path);
        } else if let Some(path) = node.settings.get("path").and_then(|v| v.as_str()) {
            proxy["path"] = serde_json::json!(path);
        }
        if let Some(host) = node.settings.get("xhttp_host").and_then(|v| v.as_str()) {
            proxy["host"] = serde_json::json!(host);
        } else if let Some(host) = node.settings.get("host").and_then(|v| v.as_str()) {
            proxy["host"] = serde_json::json!(host);
        }
    }

    if node.protocol == ProtocolType::VlessReality {
        let public_key = node
            .settings
            .get("public_key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PanelError::Subscription("missing REALITY public_key".into()))?;
        let server_name = node
            .settings
            .get("server_names")
            .and_then(|v| v.as_str())
            .or_else(|| {
                node.settings
                    .get("reality_server_names")
                    .and_then(|v| v.as_str())
            })
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
            .or_else(|| {
                node.settings
                    .get("reality_short_id")
                    .and_then(|v| v.as_str())
            })
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

        proxy["tls"] = serde_json::json!(true);
        proxy["servername"] = serde_json::json!(server_name);
        proxy["client-fingerprint"] = serde_json::json!(fingerprint);
        let mut reality = serde_json::json!({
            "enabled": true,
            "public-key": public_key,
        });
        if !short_id.is_empty() {
            reality["short-id"] = serde_json::json!(short_id);
        }
        if !spider_x.is_empty() {
            reality["spider-x"] = serde_json::json!(spider_x);
        }
        proxy["reality-opts"] = reality;
    } else if let Some(tls) = &node.tls {
        proxy["tls"] = serde_json::json!(true);
        if let Some(sni) = tls.get("serverName").and_then(|v| v.as_str()) {
            proxy["servername"] = serde_json::json!(sni);
        }
    }

    Ok(proxy)
}

fn build_vmess_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let id = node
        .settings
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing vmess id".into()))?;
    Ok(serde_json::json!({
        "name": node.name,
        "type": "vmess",
        "server": node.server,
        "port": node.port,
        "uuid": id,
        "alterId": node.settings.get("alterId").and_then(|v| v.as_u64()).unwrap_or(0),
        "cipher": node.settings.get("security").and_then(|v| v.as_str()).unwrap_or("auto"),
    }))
}

fn build_trojan_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
    let password = node
        .settings
        .get("password")
        .and_then(|v| v.as_str())
        .ok_or_else(|| PanelError::Subscription("missing trojan password".into()))?;
    let mut proxy = serde_json::json!({
        "name": node.name,
        "type": "trojan",
        "server": node.server,
        "port": node.port,
        "password": password,
    });
    if let Some(tls) = &node.tls {
        proxy["tls"] = serde_json::json!(true);
        if let Some(sni) = tls.get("serverName").and_then(|v| v.as_str()) {
            proxy["sni"] = serde_json::json!(sni);
        }
    }
    Ok(proxy)
}

fn build_shadowsocks_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
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
        "name": node.name,
        "type": "ss",
        "server": node.server,
        "port": node.port,
        "cipher": method,
        "password": password,
    }))
}

fn build_hysteria2_proxy(node: &ProxyNode) -> Result<Value, PanelError> {
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
        .ok_or_else(|| PanelError::Subscription("missing hysteria2 password".into()))?;

    let server_name = node
        .tls
        .as_ref()
        .and_then(|t| t.get("serverName"))
        .and_then(|v| v.as_str())
        .or_else(|| node.settings.get("sni").and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string();

    let mut proxy = serde_json::json!({
        "name": node.name,
        "type": "hysteria2",
        "server": node.server,
        "port": node.port,
        "password": password,
        "sni": server_name,
        "skip-cert-verify": true,
    });

    if let Some(up) = node.settings.get("up_mbps").and_then(|v| v.as_u64()) {
        proxy["up"] = serde_json::json!(up);
    }
    if let Some(down) = node.settings.get("down_mbps").and_then(|v| v.as_u64()) {
        proxy["down"] = serde_json::json!(down);
    }
    if let Some(obfs_type) = node.settings.get("obfs_type").and_then(|v| v.as_str()) {
        if obfs_type != "none" {
            if let Some(obfs_password) = node.settings.get("obfs_password").and_then(|v| v.as_str())
            {
                proxy["obfs"] = serde_json::json!(obfs_type);
                proxy["obfs-password"] = serde_json::json!(obfs_password);
            }
        }
    }

    Ok(proxy)
}
