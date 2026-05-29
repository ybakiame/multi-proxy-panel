use pp_common::PanelResult;
use serde_json::Value;

use crate::generator::ProxyNode;

/// Generate sing-box JSON subscription with outbounds array.
pub fn generate(nodes: &[ProxyNode], base_config: Option<&Value>) -> PanelResult<String> {
    let outbounds: Vec<_> = nodes
        .iter()
        .map(|node| {
            serde_json::json!({
                "type": node.protocol.to_string(),
                "tag": node.name,
                "server": node.server,
                "server_port": node.port,
            })
        })
        .collect();

    let mut config = if let Some(base) = base_config {
        base.clone()
    } else {
        serde_json::json!({ "outbounds": [] })
    };

    config["outbounds"] = serde_json::Value::Array(outbounds);
    Ok(serde_json::to_string_pretty(&config)?)
}
