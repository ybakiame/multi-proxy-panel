use pp_common::PanelResult;
use serde_json::Value;

/// Represents an online user detected from a proxy core.
#[derive(Debug, Clone)]
pub struct OnlineUser {
    pub client_id: String,
    pub email: String,
    pub ip_address: String,
    pub inbound_tag: Option<String>,
}

/// Query online users from sing-box HTTP API.
/// Returns empty list if the API is unreachable.
pub async fn query_singbox_online_users() -> PanelResult<Vec<OnlineUser>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = match client
        .get("http://127.0.0.1:9090/connections")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("sing-box connections API unreachable: {}", e);
            return Ok(vec![]);
        }
    };

    if !resp.status().is_success() {
        tracing::debug!(
            "sing-box connections API returned status {}",
            resp.status()
        );
        return Ok(vec![]);
    }

    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse sing-box connections response: {}", e);
            return Ok(vec![]);
        }
    };

    let mut users = Vec::new();

    if let Some(connections) = body.get("connections").and_then(|v| v.as_array()) {
        for conn in connections {
            let metadata = conn.get("metadata");
            let ip = metadata
                .and_then(|m| m.get("sourceIP"))
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0.0")
                .to_string();

            // For sing-box, we can try to extract the user from metadata
            // The metadata may contain a "user" field for authenticated protocols
            let email = metadata
                .and_then(|m| m.get("user"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if email.is_empty() {
                // Try to use destination as a fallback identifier
                continue;
            }

            let inbound = metadata
                .and_then(|m| m.get("inbound"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            users.push(OnlineUser {
                client_id: email.clone(),
                email,
                ip_address: ip,
                inbound_tag: inbound,
            });
        }
    }

    Ok(users)
}

/// Query online users from xray StatsService gRPC / HTTP fallback.
/// Returns empty list if the API is unreachable.
pub async fn query_xray_online_users() -> PanelResult<Vec<OnlineUser>> {
    // Xray does not expose a direct "online users" API.
    // The StatsService (gRPC on 127.0.0.1:8080) provides traffic counters keyed by email.
    // We would need a full gRPC client to QueryStats.
    // For now, return empty list as a placeholder.
    // TODO: implement tonic gRPC client for xray StatsService
    Ok(vec![])
}

/// Query online users from all available cores via the supervisor.
/// Aggregates results from sing-box and xray.
pub async fn query_all_online_users() -> PanelResult<Vec<OnlineUser>> {
    let mut all = Vec::new();

    match query_singbox_online_users().await {
        Ok(mut users) => all.append(&mut users),
        Err(e) => tracing::warn!("sing-box online user query failed: {}", e),
    }

    match query_xray_online_users().await {
        Ok(mut users) => all.append(&mut users),
        Err(e) => tracing::warn!("xray online user query failed: {}", e),
    }

    Ok(all)
}
