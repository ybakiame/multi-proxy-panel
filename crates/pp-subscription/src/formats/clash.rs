use pp_common::PanelResult;
use serde_json::Value;

use crate::generator::ProxyNode;

/// Generate Clash Meta / Mihomo YAML subscription.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&Value>) -> PanelResult<String> {
    let mut proxies = Vec::new();
    let mut proxy_names = Vec::new();

    for node in nodes {
        proxy_names.push(node.name.clone());
        proxies.push(serde_json::json!({
            "name": node.name,
            "type": node.protocol.to_string(),
            "server": node.server,
            "port": node.port,
        }));
    }

    let output = serde_json::json!({
        "proxies": proxies,
        "proxy-groups": [
            {
                "name": "Proxy",
                "type": "select",
                "proxies": proxy_names
            }
        ]
    });

    if let Some(base) = base_config {
        let _ = base;
        // TODO: merge base config
    }

    Ok(serde_json::to_string_pretty(&output)?)
}
