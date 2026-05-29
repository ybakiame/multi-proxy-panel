use pp_common::PanelResult;
use serde_json::json;

use crate::generator::ProxyNode;

/// Generate a simple JSON array of outbounds (sing-box compatible style).
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    let outbounds: Vec<_> = nodes
        .iter()
        .map(|node| {
            json!({
                "type": node.protocol.to_string(),
                "tag": node.name,
                "server": node.server,
                "server_port": node.port,
                "settings": node.settings,
            })
        })
        .collect();

    Ok(serde_json::to_string_pretty(&outbounds)?)
}
