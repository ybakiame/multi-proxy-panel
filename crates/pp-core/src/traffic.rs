/// Traffic statistics collection from core APIs.
use pp_common::PanelResult;
use serde_json::Value;

/// Parse traffic stats from xray API response.
pub fn parse_xray_stats(api_response: &Value) -> PanelResult<Vec<TrafficEntry>> {
    let mut entries = Vec::new();

    if let Some(stats) = api_response.get("stat").and_then(|v| v.as_array()) {
        for stat in stats {
            let name = stat.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let value = stat.get("value").and_then(|v| v.as_i64()).unwrap_or(0);

            // xray stat names: "inbound>>>tag>>>traffic>>>uplink"
            let parts: Vec<&str> = name.split(">>>").collect();
            if parts.len() >= 4 {
                entries.push(TrafficEntry {
                    tag: parts[1].to_string(),
                    direction: parts[3].to_string(), // uplink / downlink
                    bytes: value,
                });
            }
        }
    }

    Ok(entries)
}

/// Parse traffic stats from sing-box API response.
pub fn parse_singbox_stats(api_response: &Value) -> PanelResult<Vec<TrafficEntry>> {
    let mut entries = Vec::new();

    if let Some(inbounds) = api_response.get("inbounds").and_then(|v| v.as_object()) {
        for (tag, data) in inbounds {
            if let Some(up) = data.get("upload").and_then(|v| v.as_i64()) {
                entries.push(TrafficEntry {
                    tag: tag.clone(),
                    direction: "uplink".to_string(),
                    bytes: up,
                });
            }
            if let Some(down) = data.get("download").and_then(|v| v.as_i64()) {
                entries.push(TrafficEntry {
                    tag: tag.clone(),
                    direction: "downlink".to_string(),
                    bytes: down,
                });
            }
        }
    }

    Ok(entries)
}

#[derive(Debug, Clone)]
pub struct TrafficEntry {
    pub tag: String,
    pub direction: String,
    pub bytes: i64,
}
