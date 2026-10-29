use pp_common::PanelResult;

use crate::generator::ProxyNode;

/// Generate the same Base64-encoded share-link subscription used for V2RayNG.
pub fn generate(nodes: &[ProxyNode]) -> PanelResult<String> {
    crate::formats::base64::generate(nodes)
}
