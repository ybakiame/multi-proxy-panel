//! Traffic, metrics and log reporter to Hub.
//!
//! Collects traffic statistics from running proxy cores and sends periodic
//! reports to the Hub via the gRPC bidirectional stream.

use pp_proto::{AgentMessage, InboundTraffic, TrafficReport, UserTraffic};
use tokio::sync::mpsc;

/// Start a periodic traffic reporter task.
///
/// Fetches traffic stats from sing-box/xray APIs every `interval_secs` seconds
/// and sends a `TrafficReport` message to the Hub via the provided channel.
pub fn spawn_traffic_reporter(
    outbound: mpsc::Sender<AgentMessage>,
    interval_secs: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));

        loop {
            ticker.tick().await;

            let report = collect_traffic_report().await;
            let msg = AgentMessage {
                payload: Some(pp_proto::agent_message::Payload::Traffic(report)),
            };

            if outbound.send(msg).await.is_err() {
                tracing::debug!("traffic reporter: channel closed, stopping");
                break;
            }
        }
    })
}

/// Collect traffic statistics from all available cores.
async fn collect_traffic_report() -> TrafficReport {
    let mut inbounds = Vec::new();
    let mut users = Vec::new();

    // Collect from sing-box
    match collect_singbox_traffic().await {
        Ok((inbound_entries, user_entries)) => {
            inbounds.extend(inbound_entries);
            users.extend(user_entries);
        }
        Err(e) => {
            tracing::trace!("sing-box traffic collection failed: {}", e);
        }
    }

    // Collect from xray
    match collect_xray_traffic().await {
        Ok((inbound_entries, user_entries)) => {
            inbounds.extend(inbound_entries);
            users.extend(user_entries);
        }
        Err(e) => {
            tracing::trace!("xray traffic collection failed: {}", e);
        }
    }

    TrafficReport {
        timestamp: chrono::Utc::now().timestamp(),
        inbounds,
        users,
    }
}

/// Collect traffic stats from sing-box HTTP API.
async fn collect_singbox_traffic() -> pp_common::PanelResult<(Vec<InboundTraffic>, Vec<UserTraffic>)>
{
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| pp_common::PanelError::Core(format!("http client: {}", e)))?;

    let resp = client
        .get("http://127.0.0.1:9090/traffic")
        .send()
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("sing-box traffic API: {}", e)))?;

    if !resp.status().is_success() {
        return Err(pp_common::PanelError::Core(format!(
            "sing-box traffic API status {}",
            resp.status()
        )));
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("sing-box traffic parse: {}", e)))?;

    let mut inbounds = Vec::new();
    let mut users = Vec::new();

    // Parse inbound-level traffic
    if let Some(inbounds_obj) = body.get("inbounds").and_then(|v| v.as_object()) {
        for (tag, data) in inbounds_obj {
            let upload = data.get("upload").and_then(|v| v.as_i64()).unwrap_or(0);
            let download = data.get("download").and_then(|v| v.as_i64()).unwrap_or(0);
            if upload > 0 || download > 0 {
                inbounds.push(InboundTraffic {
                    tag: tag.clone(),
                    upload_bytes: upload,
                    download_bytes: download,
                });
            }
        }
    }

    // Parse user-level traffic
    if let Some(users_arr) = body.get("users").and_then(|v| v.as_array()) {
        for user in users_arr {
            let client_id = user
                .get("uuid")
                .or_else(|| user.get("email"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let email = user
                .get("email")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let upload = user.get("upload").and_then(|v| v.as_i64()).unwrap_or(0);
            let download = user.get("download").and_then(|v| v.as_i64()).unwrap_or(0);
            if !client_id.is_empty() && (upload > 0 || download > 0) {
                users.push(UserTraffic {
                    client_id,
                    email,
                    upload_bytes: upload,
                    download_bytes: download,
                });
            }
        }
    }

    Ok((inbounds, users))
}

/// Collect traffic stats from xray StatsService gRPC.
async fn collect_xray_traffic() -> pp_common::PanelResult<(Vec<InboundTraffic>, Vec<UserTraffic>)> {
    use pp_proto::xray_stats::{QueryStatsRequest, stats_service_client::StatsServiceClient};
    use tonic::transport::Channel;

    let channel = match Channel::from_shared("http://127.0.0.1:8080")
        .map_err(|e| pp_common::PanelError::Core(format!("invalid xray endpoint: {}", e)))?
        .connect()
        .await
    {
        Ok(c) => c,
        Err(e) => {
            return Err(pp_common::PanelError::Core(format!(
                "xray StatsService connect: {}",
                e
            )));
        }
    };

    let mut client = StatsServiceClient::new(channel);

    // Query inbound traffic
    let inbound_request = tonic::Request::new(QueryStatsRequest {
        pattern: "inbound>>>".to_string(),
        reset: true,
    });

    let inbound_response = client
        .query_stats(inbound_request)
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("xray inbound stats: {}", e)))?
        .into_inner();

    let mut inbound_map: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();

    for stat in inbound_response.stat {
        let parts: Vec<&str> = stat.name.split(">>>").collect();
        if parts.len() >= 4 {
            let tag = parts[1].to_string();
            let direction = parts[3];
            let entry = inbound_map.entry(tag).or_default();
            match direction {
                "uplink" => entry.0 += stat.value,
                "downlink" => entry.1 += stat.value,
                _ => {}
            }
        }
    }

    let inbounds: Vec<InboundTraffic> = inbound_map
        .into_iter()
        .map(|(tag, (upload, download))| InboundTraffic {
            tag,
            upload_bytes: upload,
            download_bytes: download,
        })
        .collect();

    // Query user traffic
    let user_request = tonic::Request::new(QueryStatsRequest {
        pattern: "user>>>".to_string(),
        reset: true,
    });

    let user_response = client
        .query_stats(user_request)
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("xray user stats: {}", e)))?
        .into_inner();

    let mut user_map: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::new();

    for stat in user_response.stat {
        let parts: Vec<&str> = stat.name.split(">>>").collect();
        if parts.len() >= 4 {
            let email = parts[1].to_string();
            let direction = parts[3];
            let entry = user_map.entry(email.clone()).or_default();
            match direction {
                "uplink" => entry.0 += stat.value,
                "downlink" => entry.1 += stat.value,
                _ => {}
            }
        }
    }

    let users: Vec<UserTraffic> = user_map
        .into_iter()
        .map(|(email, (upload, download))| UserTraffic {
            client_id: email.clone(),
            email,
            upload_bytes: upload,
            download_bytes: download,
        })
        .collect();

    Ok((inbounds, users))
}
