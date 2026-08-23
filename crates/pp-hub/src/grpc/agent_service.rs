use chrono::Timelike;
use pp_db::entities::{
    agent_log, client, client_online_session, node, node_binding, node_user_usage_record,
    protocol_config, traffic_record,
};
use pp_proto::{
    AgentMessage, CoreStatusReport, Heartbeat, HostMetrics, HubMessage, LogBatch,
    OnlineUsersReport, RegisterRequest, RegisterResponse, TrafficReport,
    hub_agent_server::HubAgent,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use uuid::Uuid;

use crate::state::AppState;

/// Returns true if the address is a loopback address (IPv4 or IPv6).
fn is_loopback_addr(addr: &str) -> bool {
    addr.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

/// gRPC service implementation for Hub-Agent communication.
pub struct HubAgentService {
    state: Arc<AppState>,
}

impl HubAgentService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }

    #[cfg(test)]
    pub async fn test_handle_register(
        &self,
        req: RegisterRequest,
        hub_tx: mpsc::Sender<HubMessage>,
        remote_addr: Option<String>,
    ) -> Result<Uuid, Status> {
        handle_register(&self.state, req, &hub_tx, remote_addr).await
    }

    #[cfg(test)]
    pub async fn test_handle_traffic(
        &self,
        agent_id: Uuid,
        traffic: TrafficReport,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        handle_traffic(&self.state, agent_id, traffic).await
    }

    #[cfg(test)]
    pub async fn test_handle_online_users(
        &self,
        agent_id: Uuid,
        report: OnlineUsersReport,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        handle_online_users(&self.state, agent_id, report).await
    }
}

#[tonic::async_trait]
impl HubAgent for HubAgentService {
    type StreamStream = Pin<Box<dyn Stream<Item = Result<HubMessage, Status>> + Send + 'static>>;

    async fn stream(
        &self,
        request: Request<Streaming<AgentMessage>>,
    ) -> Result<Response<Self::StreamStream>, Status> {
        // Capture peer address before consuming the request.
        let remote_addr = request.remote_addr().map(|a| a.ip().to_string());
        let mut inbound = request.into_inner();
        let state = self.state.clone();

        // (tx, rx) for sending messages from Hub to Agent
        let (tx, mut rx) = mpsc::channel::<HubMessage>(128);
        let (_registered_tx, _registered_rx) = tokio::sync::oneshot::channel::<Uuid>();

        // Spawn a task to handle inbound messages from this Agent
        let _inbound_handle = tokio::spawn(async move {
            let mut registered_id: Option<Uuid> = None;

            while let Some(msg_result) = inbound.next().await {
                match msg_result {
                    Ok(msg) => {
                        if let Err(e) = handle_agent_message(
                            &state,
                            msg,
                            &mut registered_id,
                            &tx,
                            remote_addr.clone(),
                        )
                        .await
                        {
                            tracing::warn!("error handling agent message: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("agent stream error: {}", e);
                        break;
                    }
                }
            }

            // Cleanup on disconnect
            if let Some(id) = registered_id {
                state.unregister_agent(id).await;
                if let Err(e) = update_node_offline(&state.db, id).await {
                    tracing::warn!("failed to mark node offline: {}", e);
                }
                if let Err(e) = clear_node_online_sessions(&state.db, id).await {
                    tracing::warn!("failed to clear online sessions: {}", e);
                }
            }
        });

        // The outbound stream sends messages from rx (Hub -> Agent)
        let outbound_stream = async_stream::try_stream! {
            while let Some(msg) = rx.recv().await {
                yield msg;
            }
        };

        Ok(Response::new(
            Box::pin(outbound_stream) as Self::StreamStream
        ))
    }
}

async fn handle_agent_message(
    state: &AppState,
    msg: AgentMessage,
    registered_id: &mut Option<Uuid>,
    hub_tx: &mpsc::Sender<HubMessage>,
    remote_addr: Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use pp_proto::agent_message::Payload;

    match msg.payload {
        Some(Payload::Register(req)) => {
            let agent_id = handle_register(state, req, hub_tx, remote_addr).await?;
            *registered_id = Some(agent_id);
        }
        Some(Payload::Heartbeat(hb)) => {
            if let Some(id) = registered_id {
                handle_heartbeat(state, *id, hb).await?;
            }
        }
        Some(Payload::Traffic(traffic)) => {
            if let Some(id) = registered_id {
                handle_traffic(state, *id, traffic).await?;
            }
        }
        Some(Payload::Metrics(metrics)) => {
            if let Some(id) = registered_id {
                handle_metrics(state, *id, metrics).await?;
            }
        }
        Some(Payload::Logs(logs)) => {
            if let Some(id) = registered_id {
                handle_logs(state, *id, logs).await?;
            }
        }
        Some(Payload::OnlineUsers(report)) => {
            if let Some(id) = registered_id {
                handle_online_users(state, *id, report).await?;
            }
        }
        Some(Payload::CoreStatus(status)) => {
            if let Some(id) = registered_id {
                handle_core_status(state, *id, status).await?;
            }
        }
        Some(Payload::CertStatus(report)) => {
            crate::routes::certificates::apply_cert_status(state, report).await?;
        }
        Some(Payload::CoreBinaries(list)) => {
            if let Some(id) = registered_id {
                let waiter = state.binary_waiters.write().await.remove(id);
                if let Some(tx) = waiter {
                    let _ = tx.send(list);
                }
            }
        }
        None => {}
    }

    Ok(())
}

async fn handle_register(
    state: &AppState,
    req: RegisterRequest,
    hub_tx: &mpsc::Sender<HubMessage>,
    remote_addr: Option<String>,
) -> Result<Uuid, Status> {
    let agent_id = Uuid::parse_str(&req.agent_id)
        .map_err(|e| Status::invalid_argument(format!("invalid agent_id: {}", e)))?;

    let node = node::Entity::find()
        .filter(node::Column::Id.eq(agent_id))
        .one(&state.db)
        .await
        .map_err(|e| Status::internal(format!("database error: {}", e)))?;

    let auto_register = state.config.auto_register_agents;

    if let Some(node) = node {
        // Existing node: verify token against stored hash using Argon2 verification.
        if node.token_hash.is_empty() {
            tracing::warn!(
                "node {} has empty token_hash; reject registration",
                agent_id
            );
            return Err(Status::failed_precondition(
                "node token is not set; provision a token first",
            ));
        }
        let token_valid =
            pp_common::verify_secret_async(req.token.clone(), node.token_hash.clone())
                .await
                .unwrap_or(false);
        if !token_valid {
            tracing::warn!("agent {} provided invalid token", agent_id);
            return Err(Status::unauthenticated("invalid agent token"));
        }

        let mut active: node::ActiveModel = node.into();
        active.status = Set("online".to_string());
        active.hostname = Set(req.hostname.clone());
        active.cores_available = Set(serde_json::json!(req.capabilities));
        if let Some(addr) = &remote_addr
            && !is_loopback_addr(addr)
        {
            active.address = Set(addr.clone());
        }
        if !req.domain.is_empty() {
            active.domain = Set(Some(req.domain.clone()));
        }
        active.last_seen_at = Set(Some(chrono::Utc::now().into()));
        active.updated_at = Set(chrono::Utc::now().into());
        active
            .update(&state.db)
            .await
            .map_err(|e| Status::internal(format!("database error: {}", e)))?;
    } else if auto_register {
        let token_hash = pp_common::hash_secret_async(req.token.clone())
            .await
            .map_err(|_| Status::internal("failed to hash agent token"))?;

        let new_node = node::ActiveModel {
            id: Set(agent_id),
            name: Set(req.hostname.clone()),
            hostname: Set(req.hostname.clone()),
            address: Set(remote_addr
                .clone()
                .filter(|a| !is_loopback_addr(a))
                .unwrap_or_default()),
            domain: Set(if req.domain.is_empty() {
                None
            } else {
                Some(req.domain.clone())
            }),
            token_hash: Set(token_hash),
            cores_available: Set(serde_json::json!(req.capabilities)),
            labels: Set(Some(serde_json::json!(
                req.labels
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>()
            ))),
            usage_coefficient: Set(1.0),
            status: Set("online".to_string()),
            parent_id: Set(None),
            last_seen_at: Set(Some(chrono::Utc::now().into())),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        };
        new_node
            .insert(&state.db)
            .await
            .map_err(|e| Status::internal(format!("database error: {}", e)))?;
        tracing::info!("auto-registered new agent: {}", agent_id);
    } else {
        tracing::warn!(
            "agent {} attempted to register but node does not exist",
            agent_id
        );
        return Err(Status::not_found(
            "node not registered; create the node and provision a token first",
        ));
    }

    // Register the connection
    state
        .register_agent(agent_id, hub_tx.clone(), req.core_config_versions)
        .await;

    // Dispatch any certificate issuances queued while the agent was offline.
    if let Err(e) = crate::routes::certificates::dispatch_pending_for_node(state, agent_id).await {
        tracing::warn!(
            "failed to dispatch pending certificates for {}: {}",
            agent_id,
            e
        );
    }

    // Send register response
    let resp = HubMessage {
        payload: Some(pp_proto::hub_message::Payload::RegisterResp(
            RegisterResponse {
                success: true,
                message: "registered".to_string(),
                heartbeat_interval_sec: 30,
                assigned_agent_id: agent_id.to_string(),
            },
        )),
    };
    hub_tx
        .send(resp)
        .await
        .map_err(|e| Status::internal(format!("failed to send register response: {}", e)))?;

    Ok(agent_id)
}

async fn handle_heartbeat(
    state: &AppState,
    agent_id: Uuid,
    hb: Heartbeat,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!("heartbeat from {} at {}", agent_id, hb.timestamp);

    let node = node::Entity::find_by_id(agent_id).one(&state.db).await?;
    if let Some(node) = node {
        let mut active: node::ActiveModel = node.into();
        active.last_seen_at = Set(Some(chrono::Utc::now().into()));
        active.update(&state.db).await?;
    }

    Ok(())
}

/// Resolve every reported user identifier (client UUID or email) with at
/// most one email lookup query instead of one query per user.
async fn resolve_client_ids(
    db: &sea_orm::DatabaseConnection,
    raws: &[String],
) -> std::collections::HashMap<String, Uuid> {
    let mut resolved = std::collections::HashMap::with_capacity(raws.len());
    let mut emails: Vec<String> = Vec::new();
    for raw in raws {
        if let Ok(id) = Uuid::parse_str(raw) {
            resolved.insert(raw.clone(), id);
        } else {
            emails.push(raw.clone());
        }
    }
    if !emails.is_empty() {
        match client::Entity::find()
            .filter(client::Column::Email.is_in(emails))
            .all(db)
            .await
        {
            Ok(clients) => {
                for c in clients {
                    if let Some(email) = c.email {
                        resolved.insert(email, c.id);
                    }
                }
            }
            Err(e) => tracing::warn!("failed to batch-resolve client emails: {}", e),
        }
    }
    resolved
}

async fn handle_traffic(
    state: &AppState,
    agent_id: Uuid,
    traffic: TrafficReport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!(
        "traffic report from {}: {} inbounds, {} users",
        agent_id,
        traffic.inbounds.len(),
        traffic.users.len()
    );

    let hour = chrono::Utc::now()
        .with_minute(0)
        .and_then(|d| d.with_second(0))
        .and_then(|d| d.with_nanosecond(0))
        .unwrap_or_else(chrono::Utc::now);

    // Look up the node to get its usage_coefficient (traffic rate multiplier)
    let node_model = node::Entity::find_by_id(agent_id).one(&state.db).await?;
    let rate = node_model
        .as_ref()
        .map(|n| n.usage_coefficient)
        .unwrap_or(1.0);

    // 1. Persist per-inbound traffic to traffic_records (hourly aggregated
    // upsert), resolving each reported tag back to its protocol config.
    if !traffic.inbounds.is_empty() {
        let tag_map = inbound_tag_map(state, agent_id).await;

        for inbound in &traffic.inbounds {
            let protocol_config_id = tag_map.get(inbound.tag.as_str()).copied();

            let mut query = traffic_record::Entity::find()
                .filter(traffic_record::Column::NodeId.eq(agent_id))
                .filter(traffic_record::Column::ClientId.is_null())
                .filter(traffic_record::Column::HourBucket.eq(hour));
            query = match protocol_config_id {
                Some(id) => query.filter(traffic_record::Column::ProtocolConfigId.eq(id)),
                None => query.filter(traffic_record::Column::ProtocolConfigId.is_null()),
            };
            let existing = query.one(&state.db).await?;

            if let Some(record) = existing {
                let mut active: traffic_record::ActiveModel = record.into();
                active.upload_bytes = Set(active.upload_bytes.as_ref() + inbound.upload_bytes);
                active.download_bytes =
                    Set(active.download_bytes.as_ref() + inbound.download_bytes);
                if let Err(e) = active.update(&state.db).await {
                    tracing::warn!("failed to update inbound traffic record: {}", e);
                }
            } else {
                let active = traffic_record::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    node_id: Set(Some(agent_id)),
                    protocol_config_id: Set(protocol_config_id),
                    client_id: Set(None),
                    hour_bucket: Set(hour.into()),
                    upload_bytes: Set(inbound.upload_bytes),
                    download_bytes: Set(inbound.download_bytes),
                    created_at: Set(chrono::Utc::now().into()),
                };
                if let Err(e) = active.insert(&state.db).await {
                    tracing::warn!("failed to insert inbound traffic record: {}", e);
                }
            }
        }
    }

    // 2. Persist user-level traffic to node_user_usage_records and update client stats
    let raws: Vec<String> = traffic.users.iter().map(|u| u.client_id.clone()).collect();
    let resolved = resolve_client_ids(&state.db, &raws).await;

    // Prefetch every referenced client once to avoid per-user lookups.
    let client_ids: Vec<Uuid> = resolved.values().copied().collect();
    let client_map: std::collections::HashMap<Uuid, client::Model> = if client_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        client::Entity::find()
            .filter(client::Column::Id.is_in(client_ids))
            .all(&state.db)
            .await?
            .into_iter()
            .map(|c| (c.id, c))
            .collect()
    };

    for user in &traffic.users {
        let Some(client_id) = resolved.get(&user.client_id).copied() else {
            continue;
        };

        // Apply rate multiplier: counted bytes = real bytes × rate
        let counted_upload = (user.upload_bytes as f32 * rate) as i64;
        let counted_download = (user.download_bytes as f32 * rate) as i64;

        // Upsert node_user_usage_records
        let existing = node_user_usage_record::Entity::find()
            .filter(node_user_usage_record::Column::NodeId.eq(agent_id))
            .filter(node_user_usage_record::Column::ClientId.eq(client_id))
            .filter(node_user_usage_record::Column::HourBucket.eq(hour))
            .one(&state.db)
            .await?;

        if let Some(record) = existing {
            let mut active: node_user_usage_record::ActiveModel = record.into();
            active.upload_bytes = Set(active.upload_bytes.as_ref() + counted_upload);
            active.download_bytes = Set(active.download_bytes.as_ref() + counted_download);
            if let Err(e) = active.update(&state.db).await {
                tracing::warn!("failed to update node_user_usage record: {}", e);
            }
        } else {
            let active = node_user_usage_record::ActiveModel {
                id: Set(Uuid::new_v4()),
                node_id: Set(agent_id),
                client_id: Set(client_id),
                hour_bucket: Set(hour.into()),
                upload_bytes: Set(counted_upload),
                download_bytes: Set(counted_download),
                rate: Set(rate),
                created_at: Set(chrono::Utc::now().into()),
            };
            if let Err(e) = active.insert(&state.db).await {
                tracing::warn!("failed to insert node_user_usage record: {}", e);
            }
        }

        // Update client's traffic_used_bytes (using rate-adjusted values)
        if let Some(c) = client_map.get(&client_id) {
            // Skip on_hold clients — their traffic shouldn't count until activated
            if c.status == "on_hold" {
                continue;
            }
            let prev_used = c.traffic_used_bytes;
            let mut active: client::ActiveModel = c.clone().into();
            active.traffic_used_bytes = Set(prev_used + counted_upload + counted_download);
            if let Err(e) = active.update(&state.db).await {
                tracing::warn!("failed to update client traffic: {}", e);
            }
        }
    }

    Ok(())
}

/// Map inbound tags (`{name}-{id}`) reported by agents back to their
/// protocol config IDs for this node's bindings. Tags that don't resolve
/// (e.g. synthetic core-level tags) are kept with a NULL protocol_config_id.
async fn inbound_tag_map(
    state: &AppState,
    agent_id: Uuid,
) -> std::collections::HashMap<String, Uuid> {
    let bindings = match node_binding::Entity::find()
        .filter(node_binding::Column::NodeId.eq(agent_id))
        .all(&state.db)
        .await
    {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("failed to load bindings for tag resolution: {}", e);
            return std::collections::HashMap::new();
        }
    };
    let config_ids: Vec<Uuid> = bindings.iter().map(|b| b.protocol_config_id).collect();
    if config_ids.is_empty() {
        return std::collections::HashMap::new();
    }

    match protocol_config::Entity::find()
        .filter(protocol_config::Column::Id.is_in(config_ids))
        .all(&state.db)
        .await
    {
        Ok(configs) => configs
            .into_iter()
            .map(|c| (format!("{}-{}", c.name, c.id), c.id))
            .collect(),
        Err(e) => {
            tracing::warn!("failed to load protocol configs for tag resolution: {}", e);
            std::collections::HashMap::new()
        }
    }
}

async fn handle_metrics(
    state: &AppState,
    agent_id: Uuid,
    metrics: HostMetrics,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use pp_db::entities::host_metric;

    let active = host_metric::ActiveModel {
        id: Set(Uuid::new_v4()),
        node_id: Set(agent_id),
        timestamp: Set(chrono::DateTime::from_timestamp(metrics.timestamp, 0)
            .unwrap_or_else(chrono::Utc::now)
            .into()),
        cpu_percent: Set(metrics.cpu_percent),
        mem_used: Set(metrics.mem_used as i64),
        mem_total: Set(metrics.mem_total as i64),
        disk_used: Set(metrics.disk_used as i64),
        disk_total: Set(metrics.disk_total as i64),
        net_rx: Set(metrics.net.iter().map(|n| n.rx_bytes as i64).sum()),
        net_tx: Set(metrics.net.iter().map(|n| n.tx_bytes as i64).sum()),
        load_avg1: Set(metrics.load_avg_1),
        load_avg5: Set(metrics.load_avg_5),
        load_avg15: Set(metrics.load_avg_15),
    };
    active.insert(&state.db).await?;

    Ok(())
}

async fn handle_logs(
    state: &AppState,
    agent_id: Uuid,
    logs: LogBatch,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for entry in logs.entries {
        let active = agent_log::ActiveModel {
            id: Set(Uuid::new_v4()),
            node_id: Set(agent_id),
            level: Set(entry.level),
            target: Set(entry.target),
            message: Set(entry.message),
            fields: Set(Some(serde_json::json!(
                entry
                    .fields
                    .into_iter()
                    .collect::<std::collections::HashMap<_, _>>()
            ))),
            created_at: Set(chrono::DateTime::from_timestamp(entry.timestamp, 0)
                .unwrap_or_else(chrono::Utc::now)
                .into()),
        };
        if let Err(e) = active.insert(&state.db).await {
            tracing::warn!("failed to insert agent log: {}", e);
        }
    }

    Ok(())
}

async fn handle_core_status(
    state: &AppState,
    agent_id: Uuid,
    status: CoreStatusReport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!("agent {} core status: {:?}", agent_id, status);

    let message = if status.running {
        format!(
            "core {} v{} running, uptime {}s",
            status.core_type, status.version, status.uptime_sec
        )
    } else if status.last_error.is_empty() {
        format!(
            "core {} v{} not running (no error captured)",
            status.core_type, status.version
        )
    } else {
        format!(
            "core {} v{} stopped, last error: {}",
            status.core_type, status.version, status.last_error
        )
    };

    let active = agent_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        node_id: Set(agent_id),
        level: Set(if status.running {
            "info".to_string()
        } else if status.last_error.is_empty() {
            "warning".to_string()
        } else {
            "error".to_string()
        }),
        target: Set(format!("core-{}", status.core_type)),
        message: Set(message),
        fields: Set(Some(serde_json::json!({
            "core_type": status.core_type,
            "version": status.version,
            "running": status.running,
            "uptime_sec": status.uptime_sec,
            "active_inbounds": status.active_inbounds,
            "last_error": status.last_error,
        }))),
        created_at: Set(chrono::Utc::now().into()),
    };
    if let Err(e) = active.insert(&state.db).await {
        tracing::warn!("failed to insert core status log: {}", e);
    }

    Ok(())
}

async fn handle_online_users(
    state: &AppState,
    agent_id: Uuid,
    report: OnlineUsersReport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let now = chrono::Utc::now();

    // Load existing sessions for this node, keyed by (client_id, ip_address)
    let existing_sessions = client_online_session::Entity::find()
        .filter(client_online_session::Column::NodeId.eq(agent_id))
        .all(&state.db)
        .await?;
    let session_map: std::collections::HashMap<(Uuid, &str), &client_online_session::Model> =
        existing_sessions
            .iter()
            .map(|s| ((s.client_id, s.ip_address.as_str()), s))
            .collect();

    let mut reported_keys: std::collections::HashSet<(Uuid, String)> =
        std::collections::HashSet::new();
    let mut kept_session_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    // Upsert sessions and activate on-hold clients on first connection
    let raws: Vec<String> = report.users.iter().map(|u| u.client_id.clone()).collect();
    let resolved = resolve_client_ids(&state.db, &raws).await;

    // Prefetch referenced clients once for on-hold activation checks.
    let client_ids: Vec<Uuid> = resolved.values().copied().collect();
    let client_map: std::collections::HashMap<Uuid, client::Model> = if client_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        client::Entity::find()
            .filter(client::Column::Id.is_in(client_ids))
            .all(&state.db)
            .await?
            .into_iter()
            .map(|c| (c.id, c))
            .collect()
    };

    for user in &report.users {
        let Some(client_id) = resolved.get(&user.client_id).copied() else {
            continue;
        };

        // Skip duplicates of the same (client_id, ip_address) within one report
        if !reported_keys.insert((client_id, user.ip_address.clone())) {
            continue;
        }

        // Activate on-hold clients on first actual connection
        if let Some(c) = client_map.get(&client_id)
            && c.status == "on_hold"
        {
            let now = chrono::Utc::now();
            let expire_date = c
                .on_hold_expire_duration_secs
                .map(|secs| now + chrono::Duration::seconds(secs));

            let mut active: client::ActiveModel = c.clone().into();
            active.status = Set("active".to_string());
            active.expiry_date = Set(expire_date.map(|d| d.into()));
            active.updated_at = Set(now.into());
            if let Err(e) = active.update(&state.db).await {
                tracing::warn!("failed to activate on-hold client {}: {}", client_id, e);
            } else {
                tracing::info!("activated on-hold client {} on first connection", client_id);

                let _ = crate::service::webhook::trigger_event(
                    &state.db,
                    "client_activated",
                    &serde_json::json!({
                        "client_id": client_id,
                        "activated_at": now.to_rfc3339(),
                        "expire_date": expire_date.map(|d| d.to_rfc3339()),
                    }),
                )
                .await;
            }
        }

        let key = (client_id, user.ip_address.as_str());
        if let Some(session) = session_map.get(&key) {
            // Existing session: only refresh last_active_at, keep connected_at
            kept_session_ids.insert(session.id);
            let mut active: client_online_session::ActiveModel = (*session).clone().into();
            active.last_active_at = Set(now.into());
            if let Err(e) = active.update(&state.db).await {
                tracing::warn!("failed to refresh online session {}: {}", session.id, e);
            }
        } else {
            let active = client_online_session::ActiveModel {
                id: Set(Uuid::new_v4()),
                client_id: Set(client_id),
                node_id: Set(agent_id),
                ip_address: Set(user.ip_address.clone()),
                inbound_tag: Set(if user.inbound_tag.is_empty() {
                    None
                } else {
                    Some(user.inbound_tag.clone())
                }),
                connected_at: Set(chrono::DateTime::from_timestamp(report.timestamp, 0)
                    .unwrap_or_else(chrono::Utc::now)
                    .into()),
                last_active_at: Set(now.into()),
            };
            match active.insert(&state.db).await {
                Ok(model) => {
                    kept_session_ids.insert(model.id);
                }
                Err(e) => {
                    tracing::warn!("failed to insert online session: {}", e);
                }
            }
        }
    }

    // Remove sessions that were not reported in this round
    let mut delete = client_online_session::Entity::delete_many()
        .filter(client_online_session::Column::NodeId.eq(agent_id));
    if !kept_session_ids.is_empty() {
        delete = delete.filter(client_online_session::Column::Id.is_not_in(kept_session_ids));
    }
    delete.exec(&state.db).await?;

    tracing::debug!(
        "online users report from {}: {} users",
        agent_id,
        report.users.len()
    );
    Ok(())
}

async fn update_node_offline(
    db: &sea_orm::DatabaseConnection,
    agent_id: Uuid,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let node = node::Entity::find_by_id(agent_id).one(db).await?;
    if let Some(node) = node {
        let mut active: node::ActiveModel = node.into();
        active.status = Set("offline".to_string());
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(db).await?;
    }
    Ok(())
}

/// Delete all online sessions of a node whose agent disconnected.
async fn clear_node_online_sessions(
    db: &sea_orm::DatabaseConnection,
    agent_id: Uuid,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let res = client_online_session::Entity::delete_many()
        .filter(client_online_session::Column::NodeId.eq(agent_id))
        .exec(db)
        .await?;
    if res.rows_affected > 0 {
        tracing::debug!(
            "cleared {} online sessions for node {}",
            res.rows_affected,
            agent_id
        );
    }
    Ok(())
}
