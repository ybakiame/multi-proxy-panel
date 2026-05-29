use pp_common::PanelResult;

use crate::generator::ProxyNode;

/// V2RayNG format is essentially the same as base64 vmess/vless links.
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    crate::formats::base64::generate(nodes)
}
