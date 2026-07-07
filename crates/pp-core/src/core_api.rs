use pp_common::PanelResult;
use serde_json::Value;
use std::collections::HashSet;

/// Represents an online user detected from a proxy core.
#[derive(Debug, Clone)]
pub struct OnlineUser {
    pub client_id: String,
    pub email: String,
    pub ip_address: String,
    pub inbound_tag: Option<String>,
}

/// Query online users from sing-box.
/// Tries the gRPC API first (new in 1.14.0-alpha.30), then falls back to the
/// legacy HTTP REST API on `127.0.0.1:9090/connections`.
pub async fn query_singbox_online_users() -> PanelResult<Vec<OnlineUser>> {
    match query_singbox_online_users_grpc().await {
        Ok(users) => return Ok(users),
        Err(e) => {
            tracing::debug!(
                "sing-box gRPC connections API unavailable ({}), falling back to HTTP",
                e
            );
        }
    }
    query_singbox_online_users_http().await
}

/// Query online users from sing-box gRPC StartedService::SubscribeConnections.
async fn query_singbox_online_users_grpc() -> PanelResult<Vec<OnlineUser>> {
    use pp_proto::singbox_daemon::{
        SubscribeConnectionsRequest, started_service_client::StartedServiceClient,
    };
    use tonic::metadata::MetadataValue;
    use tonic::transport::Channel;

    let endpoint = std::env::var("PROXYPANEL_SINGBOX_API_LISTEN")
        .unwrap_or_else(|_| "http://127.0.0.1:9090".to_string());
    let endpoint = if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint
    } else {
        format!("http://{}", endpoint)
    };

    let channel = Channel::from_shared(endpoint.clone())
        .map_err(|e| pp_common::PanelError::Core(format!("invalid sing-box gRPC endpoint: {}", e)))?
        .connect()
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("sing-box gRPC connect failed: {}", e)))?;

    let mut client = StartedServiceClient::new(channel);
    let mut request = tonic::Request::new(SubscribeConnectionsRequest { interval: 0 });
    if let Ok(secret) = std::env::var("PROXYPANEL_SINGBOX_API_SECRET") {
        if !secret.is_empty() {
            let token = format!("Bearer {}", secret);
            let metadata = MetadataValue::try_from(token)
                .map_err(|e| pp_common::PanelError::Core(format!("invalid api secret: {}", e)))?;
            request.metadata_mut().insert("authorization", metadata);
        }
    }

    let response = client
        .subscribe_connections(request)
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("subscribe_connections failed: {}", e)))?;

    let mut stream = response.into_inner();
    let first = stream
        .message()
        .await
        .map_err(|e| {
            pp_common::PanelError::Core(format!("sing-box connection stream error: {}", e))
        })?
        .ok_or_else(|| pp_common::PanelError::Core("sing-box connection stream closed".into()))?;

    let mut users = Vec::new();
    for event in first.events {
        if event.r#type == 2 {
            // CONNECTION_EVENT_CLOSED
            continue;
        }
        let Some(conn) = event.connection else {
            continue;
        };
        if conn.user.is_empty() {
            continue;
        }
        users.push(OnlineUser {
            client_id: conn.user.clone(),
            email: conn.user,
            ip_address: conn.source,
            inbound_tag: Some(conn.inbound),
        });
    }

    Ok(users)
}

/// Query online users from sing-box HTTP API.
/// Returns empty list if the API is unreachable.
async fn query_singbox_online_users_http() -> PanelResult<Vec<OnlineUser>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let resp = match client.get("http://127.0.0.1:9090/connections").send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("sing-box connections API unreachable: {}", e);
            return Ok(vec![]);
        }
    };

    if !resp.status().is_success() {
        tracing::debug!("sing-box connections API returned status {}", resp.status());
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

/// Query online users from xray StatsService gRPC.
/// Returns empty list if the API is unreachable.
pub async fn query_xray_online_users() -> PanelResult<Vec<OnlineUser>> {
    use pp_proto::xray_stats::{QueryStatsRequest, stats_service_client::StatsServiceClient};
    use tonic::transport::Channel;

    let channel = match Channel::from_shared("http://127.0.0.1:8080")
        .map_err(|e| pp_common::PanelError::Core(format!("invalid xray gRPC endpoint: {}", e)))?
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            tracing::debug!("xray StatsService unreachable: {}", e);
            return Ok(vec![]);
        }
    };

    let mut client = StatsServiceClient::new(channel);
    let request = tonic::Request::new(QueryStatsRequest {
        pattern: "user>>".to_string(),
        reset: false,
    });

    let response = match client.query_stats(request).await {
        Ok(resp) => resp.into_inner(),
        Err(e) => {
            tracing::debug!("xray query_stats failed: {}", e);
            return Ok(vec![]);
        }
    };

    let mut emails = HashSet::new();
    for stat in response.stat {
        if stat.value <= 0 {
            continue;
        }
        if let Some(email) = parse_xray_stat_email(&stat.name) {
            emails.insert(email);
        }
    }

    Ok(emails
        .into_iter()
        .map(|email| OnlineUser {
            client_id: email.clone(),
            email,
            ip_address: "0.0.0.0".to_string(),
            inbound_tag: None,
        })
        .collect())
}

/// Parse an email from an xray stat name such as `user>>example@domain.com>>>traffic>>>uplink`.
fn parse_xray_stat_email(name: &str) -> Option<String> {
    // Split into major sections: "user>>{email}" | "traffic" | "uplink"
    let parts: Vec<&str> = name.split(">>>").collect();
    let user_section = parts.first()?;
    // "user>>{email}"
    let (tag, email) = user_section.split_once(">>")?;
    if tag != "user" || email.is_empty() {
        return None;
    }
    Some(email.to_string())
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
