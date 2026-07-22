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

/// Base URL of the sing-box API listener (`PROXYPANEL_SINGBOX_API_LISTEN`,
/// default `127.0.0.1:9090`), scheme added when missing.
///
/// Must match the listen address written into the sing-box config by the Hub.
fn singbox_api_base() -> String {
    let listen = std::env::var("PROXYPANEL_SINGBOX_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9090".to_string());
    let listen = listen
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    format!("http://{}", listen)
}

/// gRPC endpoint of the xray StatsService (`PROXYPANEL_XRAY_API_LISTEN`,
/// default `127.0.0.1:8080`), scheme added when missing.
fn xray_grpc_endpoint() -> String {
    let listen = std::env::var("PROXYPANEL_XRAY_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    if listen.starts_with("http://") || listen.starts_with("https://") {
        listen
    } else {
        format!("http://{}", listen)
    }
}

/// Base URL of the mihomo Clash API (`PROXYPANEL_MIHOMO_API_LISTEN`,
/// default `127.0.0.1:9093`), scheme added when missing.
///
/// Must match the `external-controller` address written into the mihomo
/// config by the Hub.
pub fn mihomo_api_base() -> String {
    let listen = std::env::var("PROXYPANEL_MIHOMO_API_LISTEN")
        .unwrap_or_else(|_| "127.0.0.1:9093".to_string());
    let listen = listen
        .trim_start_matches("http://")
        .trim_start_matches("https://");
    format!("http://{}", listen)
}

/// Shared secret for the mihomo Clash API (`PROXYPANEL_MIHOMO_API_SECRET`).
pub fn mihomo_api_secret() -> String {
    std::env::var("PROXYPANEL_MIHOMO_API_SECRET").unwrap_or_default()
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

    let resp = match client
        .get(format!("{}/connections", singbox_api_base()))
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
///
/// Resolution order:
/// 1. `GetAllOnlineUsers` (recent Xray) for the online email list;
/// 2. `QueryStats` for `user>>>{email}>>>online` counters;
/// 3. legacy heuristic: any user with nonzero traffic counters.
///
/// For each online email, `GetStatsOnlineIpList` resolves the real client
/// IPs; when unavailable the user is reported with `0.0.0.0`.
/// Returns empty list if the API is unreachable.
pub async fn query_xray_online_users() -> PanelResult<Vec<OnlineUser>> {
    use pp_proto::xray_stats::{
        GetAllOnlineUsersRequest, GetStatsRequest, QueryStatsRequest,
        stats_service_client::StatsServiceClient,
    };
    use tonic::transport::Channel;

    let channel = match Channel::from_shared(xray_grpc_endpoint())
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

    let emails: Vec<String> = match client
        .get_all_online_users(tonic::Request::new(GetAllOnlineUsersRequest {}))
        .await
    {
        Ok(resp) => resp.into_inner().users,
        Err(e) => {
            tracing::debug!(
                "xray GetAllOnlineUsers unavailable ({}), falling back to online counters",
                e
            );
            let request = tonic::Request::new(QueryStatsRequest {
                pattern: "user>>>".to_string(),
                reset: false,
            });
            match client.query_stats(request).await {
                Ok(resp) => {
                    let stats = resp.into_inner().stat;
                    let mut emails: Vec<String> = stats
                        .iter()
                        .filter(|s| s.value > 0)
                        .filter_map(|s| parse_xray_online_stat_email(&s.name))
                        .collect::<HashSet<_>>()
                        .into_iter()
                        .collect();
                    if emails.is_empty() {
                        // Legacy heuristic: treat users with any nonzero
                        // traffic counter as online.
                        emails = stats
                            .iter()
                            .filter(|s| s.value > 0)
                            .filter_map(|s| parse_xray_traffic_stat_email(&s.name))
                            .collect::<HashSet<_>>()
                            .into_iter()
                            .collect();
                    }
                    emails
                }
                Err(e) => {
                    tracing::debug!("xray query_stats failed: {}", e);
                    return Ok(vec![]);
                }
            }
        }
    };

    let mut users = Vec::new();
    for email in emails {
        let stat_name = format!("user>>>{}>>>online", email);
        let request = tonic::Request::new(GetStatsRequest {
            name: stat_name,
            reset: false,
        });
        match client.get_stats_online_ip_list(request).await {
            Ok(resp) => {
                let ips = resp.into_inner().ips;
                if ips.is_empty() {
                    users.push(xray_online_user(email, "0.0.0.0".to_string()));
                } else {
                    for ip in ips.into_keys() {
                        users.push(xray_online_user(email.clone(), ip));
                    }
                }
            }
            Err(e) => {
                tracing::debug!("xray GetStatsOnlineIpList failed for {}: {}", email, e);
                users.push(xray_online_user(email, "0.0.0.0".to_string()));
            }
        }
    }

    Ok(users)
}

fn xray_online_user(email: String, ip_address: String) -> OnlineUser {
    OnlineUser {
        client_id: email.clone(),
        email,
        ip_address,
        inbound_tag: None,
    }
}

/// Parse an email from an xray online stat name such as
/// `user>>>example@domain.com>>>online`.
fn parse_xray_online_stat_email(name: &str) -> Option<String> {
    let rest = name.strip_prefix("user>>>")?;
    let email = rest.strip_suffix(">>>online")?;
    if email.is_empty() {
        None
    } else {
        Some(email.to_string())
    }
}

/// Parse an email from an xray traffic stat name such as
/// `user>>>example@domain.com>>>traffic>>>uplink`.
fn parse_xray_traffic_stat_email(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split(">>>").collect();
    if parts.len() >= 4 && parts[0] == "user" && !parts[1].is_empty() {
        Some(parts[1].to_string())
    } else {
        None
    }
}

/// Query online users from mihomo's Clash API `/connections`.
///
/// Only connections carrying an `inboundUser` (i.e. listeners with user
/// authentication) can be attributed to a client; the rest are skipped.
/// Returns empty list if the API is unreachable.
pub async fn query_mihomo_online_users() -> PanelResult<Vec<OnlineUser>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let mut request = client.get(format!("{}/connections", mihomo_api_base()));
    let secret = mihomo_api_secret();
    if !secret.is_empty() {
        request = request.bearer_auth(secret);
    }

    let resp = match request.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::debug!("mihomo connections API unreachable: {}", e);
            return Ok(vec![]);
        }
    };

    if !resp.status().is_success() {
        tracing::debug!("mihomo connections API returned status {}", resp.status());
        return Ok(vec![]);
    }

    let body: Value = match resp.json().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("failed to parse mihomo connections response: {}", e);
            return Ok(vec![]);
        }
    };

    let mut users = Vec::new();
    if let Some(connections) = body.get("connections").and_then(|v| v.as_array()) {
        for conn in connections {
            let Some(metadata) = conn.get("metadata") else {
                continue;
            };
            let user = metadata
                .get("inboundUser")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if user.is_empty() {
                continue;
            }
            users.push(OnlineUser {
                client_id: user.to_string(),
                email: user.to_string(),
                ip_address: metadata
                    .get("sourceIP")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0.0")
                    .to_string(),
                inbound_tag: metadata
                    .get("inboundName")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }

    Ok(users)
}

/// Query online users from all available cores via the supervisor.
/// Aggregates results from sing-box, xray and mihomo.
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

    match query_mihomo_online_users().await {
        Ok(mut users) => all.append(&mut users),
        Err(e) => tracing::warn!("mihomo online user query failed: {}", e),
    }

    Ok(all)
}
