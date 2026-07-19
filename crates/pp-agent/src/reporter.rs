//! Traffic, metrics and log reporter to Hub.
//!
//! Collects traffic statistics from running proxy cores and sends periodic
//! reports to the Hub via the gRPC bidirectional stream.

use pp_proto::singbox_daemon::ConnectionEvent;
use pp_proto::{AgentMessage, InboundTraffic, TrafficReport, UserTraffic};
use std::collections::HashMap;
use std::sync::OnceLock;
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

/// Collect traffic stats from sing-box.
///
/// Tries the sing-box 1.14.0+ gRPC API first and falls back to the legacy
/// HTTP API when gRPC is unavailable.
async fn collect_singbox_traffic() -> pp_common::PanelResult<(Vec<InboundTraffic>, Vec<UserTraffic>)>
{
    match collect_singbox_traffic_grpc().await {
        Ok(report) => return Ok(report),
        Err(e) => {
            tracing::debug!(
                "sing-box gRPC traffic collection failed ({}), falling back to HTTP",
                e
            );
        }
    }
    collect_singbox_traffic_http().await
}

/// Process-local sing-box sampling state used to turn the cumulative counters
/// exposed by the gRPC API into per-period deltas.
#[derive(Default)]
struct SingboxTrafficState {
    /// Previous `(uplink_total, downlink_total)` from `SubscribeStatus`.
    prev_total: Option<(i64, i64)>,
    /// Previous connection snapshot: connection id -> (user, uplink_total, downlink_total).
    prev_conns: HashMap<String, (String, i64, i64)>,
}

static SINGBOX_TRAFFIC_STATE: OnceLock<tokio::sync::Mutex<SingboxTrafficState>> = OnceLock::new();

fn singbox_traffic_state() -> &'static tokio::sync::Mutex<SingboxTrafficState> {
    SINGBOX_TRAFFIC_STATE.get_or_init(|| tokio::sync::Mutex::new(SingboxTrafficState::default()))
}

/// Collect traffic stats from the sing-box 1.14.0+ gRPC StartedService.
///
/// Endpoint and secret conventions match `pp_core::core_api`:
/// `PROXYPANEL_SINGBOX_API_LISTEN` (default `http://127.0.0.1:9090`) and
/// `PROXYPANEL_SINGBOX_API_SECRET` sent as a Bearer token.
async fn collect_singbox_traffic_grpc()
-> pp_common::PanelResult<(Vec<InboundTraffic>, Vec<UserTraffic>)> {
    use pp_proto::singbox_daemon::{
        SubscribeConnectionsRequest, SubscribeStatusRequest,
        started_service_client::StartedServiceClient,
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

    let channel = Channel::from_shared(endpoint)
        .map_err(|e| pp_common::PanelError::Core(format!("invalid sing-box gRPC endpoint: {}", e)))?
        .connect()
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("sing-box gRPC connect failed: {}", e)))?;

    let mut client = StartedServiceClient::new(channel);

    let secret = std::env::var("PROXYPANEL_SINGBOX_API_SECRET").unwrap_or_default();
    let auth = if secret.is_empty() {
        None
    } else {
        Some(
            MetadataValue::try_from(format!("Bearer {}", secret))
                .map_err(|e| pp_common::PanelError::Core(format!("invalid api secret: {}", e)))?,
        )
    };

    // Kernel-wide cumulative counters come from the first Status message.
    let mut status_req = tonic::Request::new(SubscribeStatusRequest { interval: 0 });
    if let Some(token) = auth.clone() {
        status_req.metadata_mut().insert("authorization", token);
    }
    let mut status_stream = client
        .subscribe_status(status_req)
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("subscribe_status failed: {}", e)))?
        .into_inner();
    let status = status_stream
        .message()
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("sing-box status stream error: {}", e)))?
        .ok_or_else(|| pp_common::PanelError::Core("sing-box status stream closed".into()))?;

    // The first ConnectionEvents message is a full snapshot of live connections.
    let mut conn_req = tonic::Request::new(SubscribeConnectionsRequest { interval: 0 });
    if let Some(token) = auth {
        conn_req.metadata_mut().insert("authorization", token);
    }
    let mut conn_stream = client
        .subscribe_connections(conn_req)
        .await
        .map_err(|e| pp_common::PanelError::Core(format!("subscribe_connections failed: {}", e)))?
        .into_inner();
    let first = conn_stream
        .message()
        .await
        .map_err(|e| {
            pp_common::PanelError::Core(format!("sing-box connection stream error: {}", e))
        })?
        .ok_or_else(|| pp_common::PanelError::Core("sing-box connection stream closed".into()))?;

    let snapshot = connection_snapshot(&first.events);

    let mut state = singbox_traffic_state().lock().await;
    let cur_total = (status.uplink_total, status.downlink_total);
    let (upload, download) = total_delta(state.prev_total, cur_total);
    state.prev_total = Some(cur_total);
    let per_user = user_deltas(&state.prev_conns, &snapshot);
    state.prev_conns = snapshot;

    let mut inbounds = Vec::new();
    if upload > 0 || download > 0 {
        inbounds.push(InboundTraffic {
            tag: "sing-box".to_string(),
            upload_bytes: upload,
            download_bytes: download,
        });
    }

    let users = per_user
        .into_iter()
        .map(|(user, (upload_bytes, download_bytes))| UserTraffic {
            client_id: user.clone(),
            email: user,
            upload_bytes,
            download_bytes,
        })
        .collect();

    Ok((inbounds, users))
}

/// Delta between the previous and current kernel-wide totals.
///
/// The first sample counts the full cumulative value, mirroring xray's
/// `reset = true` first-poll semantics. Counter regressions (e.g. after a
/// core restart) are clamped to zero.
fn total_delta(prev: Option<(i64, i64)>, cur: (i64, i64)) -> (i64, i64) {
    match prev {
        Some((up, down)) => (
            cur.0.saturating_sub(up).max(0),
            cur.1.saturating_sub(down).max(0),
        ),
        None => cur,
    }
}

/// Build a `conn.id -> (user, uplink_total, downlink_total)` snapshot from connection events.
///
/// Closed connections, events without a connection payload and connections
/// without a user are skipped: none of them can be attributed to a client.
fn connection_snapshot(events: &[ConnectionEvent]) -> HashMap<String, (String, i64, i64)> {
    let mut snapshot = HashMap::with_capacity(events.len());
    for event in events {
        if event.r#type == 2 {
            // CONNECTION_EVENT_CLOSED
            continue;
        }
        let Some(conn) = &event.connection else {
            continue;
        };
        if conn.user.is_empty() {
            continue;
        }
        snapshot.insert(
            conn.id.clone(),
            // sing-box populates the cumulative per-connection counters in
            // `uplink_total`/`downlink_total`; `uplink`/`downlink` stay zero
            // in snapshot events.
            (conn.user.clone(), conn.uplink_total, conn.downlink_total),
        );
    }
    snapshot
}

/// Aggregate per-user traffic deltas between the previous and current snapshot.
///
/// New connections contribute their full cumulative counters; existing ones
/// contribute the difference (clamped to zero to guard against counter
/// regressions). Connections missing from the current snapshot are dropped:
/// the bytes they transferred since the last sample go uncounted, which is
/// acceptable for periodic sampling.
fn user_deltas(
    prev: &HashMap<String, (String, i64, i64)>,
    snapshot: &HashMap<String, (String, i64, i64)>,
) -> HashMap<String, (i64, i64)> {
    let mut per_user: HashMap<String, (i64, i64)> = HashMap::new();
    for (id, (user, up, down)) in snapshot {
        let (dup, ddown) = match prev.get(id) {
            Some((_, prev_up, prev_down)) => (
                up.saturating_sub(*prev_up).max(0),
                down.saturating_sub(*prev_down).max(0),
            ),
            None => (*up, *down),
        };
        if dup == 0 && ddown == 0 {
            continue;
        }
        let entry = per_user.entry(user.clone()).or_default();
        entry.0 += dup;
        entry.1 += ddown;
    }
    per_user
}

/// Collect traffic stats from the legacy sing-box HTTP API.
async fn collect_singbox_traffic_http()
-> pp_common::PanelResult<(Vec<InboundTraffic>, Vec<UserTraffic>)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use pp_proto::singbox_daemon::Connection;

    const CLOSED: i32 = 2; // CONNECTION_EVENT_CLOSED
    const NEW: i32 = 0; // CONNECTION_EVENT_NEW

    fn conn_event(ty: i32, id: &str, user: &str, uplink: i64, downlink: i64) -> ConnectionEvent {
        ConnectionEvent {
            r#type: ty,
            id: id.to_string(),
            connection: Some(Connection {
                id: id.to_string(),
                user: user.to_string(),
                // sing-box fills the cumulative counters here, not in
                // `uplink`/`downlink`.
                uplink_total: uplink,
                downlink_total: downlink,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn total_delta_first_sample_counts_full_totals() {
        assert_eq!(total_delta(None, (100, 200)), (100, 200));
    }

    #[test]
    fn total_delta_second_sample_counts_difference() {
        assert_eq!(total_delta(Some((100, 200)), (150, 260)), (50, 60));
    }

    #[test]
    fn total_delta_zero_and_counter_reset() {
        assert_eq!(total_delta(Some((100, 200)), (100, 200)), (0, 0));
        assert_eq!(total_delta(Some((100, 200)), (50, 60)), (0, 0));
    }

    #[test]
    fn snapshot_skips_closed_userless_and_payloadless_events() {
        let events = vec![
            conn_event(NEW, "c1", "alice", 10, 20),
            conn_event(CLOSED, "c2", "bob", 30, 40),
            conn_event(NEW, "c3", "", 50, 60),
            ConnectionEvent {
                r#type: NEW,
                id: "c4".to_string(),
                connection: None,
                ..Default::default()
            },
        ];
        let snapshot = connection_snapshot(&events);
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot.get("c1"), Some(&("alice".to_string(), 10, 20)));
    }

    #[test]
    fn user_deltas_new_connection_counts_full_counters() {
        let prev = HashMap::new();
        let mut snapshot = HashMap::new();
        snapshot.insert("c1".to_string(), ("alice".to_string(), 100, 200));
        let deltas = user_deltas(&prev, &snapshot);
        assert_eq!(deltas.get("alice"), Some(&(100, 200)));
    }

    #[test]
    fn user_deltas_existing_connection_counts_difference() {
        let mut prev = HashMap::new();
        prev.insert("c1".to_string(), ("alice".to_string(), 100, 200));
        let mut snapshot = HashMap::new();
        snapshot.insert("c1".to_string(), ("alice".to_string(), 130, 260));
        let deltas = user_deltas(&prev, &snapshot);
        assert_eq!(deltas.get("alice"), Some(&(30, 60)));
    }

    #[test]
    fn user_deltas_counter_decrease_saturates_to_zero() {
        let mut prev = HashMap::new();
        prev.insert("c1".to_string(), ("alice".to_string(), 100, 200));
        let mut snapshot = HashMap::new();
        snapshot.insert("c1".to_string(), ("alice".to_string(), 50, 60));
        let deltas = user_deltas(&prev, &snapshot);
        assert!(deltas.is_empty());
    }

    #[test]
    fn user_deltas_zero_delta_is_skipped() {
        let mut prev = HashMap::new();
        prev.insert("c1".to_string(), ("alice".to_string(), 100, 200));
        let mut snapshot = HashMap::new();
        snapshot.insert("c1".to_string(), ("alice".to_string(), 100, 200));
        let deltas = user_deltas(&prev, &snapshot);
        assert!(deltas.is_empty());
    }

    #[test]
    fn user_deltas_aggregates_connections_of_same_user() {
        let mut prev = HashMap::new();
        prev.insert("c1".to_string(), ("alice".to_string(), 0, 0));
        let mut snapshot = HashMap::new();
        snapshot.insert("c1".to_string(), ("alice".to_string(), 10, 20));
        snapshot.insert("c2".to_string(), ("alice".to_string(), 5, 7));
        snapshot.insert("c3".to_string(), ("bob".to_string(), 1, 2));
        let deltas = user_deltas(&prev, &snapshot);
        assert_eq!(deltas.get("alice"), Some(&(15, 27)));
        assert_eq!(deltas.get("bob"), Some(&(1, 2)));
    }

    #[test]
    fn user_deltas_drops_vanished_connections() {
        let mut prev = HashMap::new();
        prev.insert("c1".to_string(), ("alice".to_string(), 100, 200));
        prev.insert("c2".to_string(), ("bob".to_string(), 100, 200));
        let mut snapshot = HashMap::new();
        snapshot.insert("c1".to_string(), ("alice".to_string(), 110, 220));
        let deltas = user_deltas(&prev, &snapshot);
        assert_eq!(deltas.get("alice"), Some(&(10, 20)));
        assert!(!deltas.contains_key("bob"));
    }
}
