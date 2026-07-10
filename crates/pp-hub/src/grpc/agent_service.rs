use chrono::Timelike;
use pp_db::entities::{
    client, client_online_session, node, node_user_usage_record, traffic_record,
};
use pp_proto::{
    AgentMessage, Heartbeat, HostMetrics, HubMessage, LogBatch, OnlineUsersReport, RegisterRequest,
    RegisterResponse, TrafficReport, hub_agent_server::HubAgent,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::{Stream, StreamExt};
use tonic::{Request, Response, Status, Streaming};
use uuid::Uuid;

use crate::state::AppState;

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
                tracing::debug!("agent {} core status: {:?}", id, status);
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
        if let Some(addr) = &remote_addr {
            active.address = Set(addr.clone());
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
            address: Set(remote_addr.clone().unwrap_or_default()),
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
    state.register_agent(agent_id, hub_tx.clone()).await;

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
        .with_second(0)
        .and_then(|d| d.with_minute(0))
        .unwrap_or_else(chrono::Utc::now);

    // Look up the node to get its usage_coefficient (traffic rate multiplier)
    let node_model = node::Entity::find_by_id(agent_id).one(&state.db).await?;
    let rate = node_model
        .as_ref()
        .map(|n| n.usage_coefficient)
        .unwrap_or(1.0);

    // 1. Persist inbound-level traffic to traffic_records
    for inbound in &traffic.inbounds {
        let active = traffic_record::ActiveModel {
            id: Set(Uuid::new_v4()),
            node_id: Set(Some(agent_id)),
            protocol_config_id: Set(None),
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

    // 2. Persist user-level traffic to node_user_usage_records and update client stats
    for user in &traffic.users {
        let client_id = Uuid::parse_str(&user.client_id).unwrap_or_else(|_| Uuid::nil());
        if client_id.is_nil() {
            continue;
        }

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
        if let Ok(Some(c)) = client::Entity::find_by_id(client_id).one(&state.db).await {
            // Skip on_hold clients — their traffic shouldn't count until activated
            if c.status == "on_hold" {
                continue;
            }
            let prev_used = c.traffic_used_bytes;
            let mut active: client::ActiveModel = c.into();
            active.traffic_used_bytes = Set(prev_used + counted_upload + counted_download);
            if let Err(e) = active.update(&state.db).await {
                tracing::warn!("failed to update client traffic: {}", e);
            }
        }
    }

    Ok(())
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
    use pp_db::entities::system_log;

    for entry in logs.entries {
        let active = system_log::ActiveModel {
            id: Set(Uuid::new_v4()),
            level: Set(entry.level),
            source: Set(format!("agent-{}", agent_id)),
            message: Set(entry.message),
            metadata: Set(Some(serde_json::json!({
                "target": entry.target,
                "fields": entry.fields,
            }))),
            created_at: Set(chrono::DateTime::from_timestamp(entry.timestamp, 0)
                .unwrap_or_else(chrono::Utc::now)
                .into()),
        };
        active.insert(&state.db).await?;
    }

    Ok(())
}

async fn handle_online_users(
    state: &AppState,
    agent_id: Uuid,
    report: OnlineUsersReport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Delete old sessions for this node
    client_online_session::Entity::delete_many()
        .filter(client_online_session::Column::NodeId.eq(agent_id))
        .exec(&state.db)
        .await?;

    // Insert new sessions and activate on-hold clients on first connection
    for user in &report.users {
        let client_id = Uuid::parse_str(&user.client_id).unwrap_or_else(|_| Uuid::nil());
        if client_id.is_nil() {
            continue;
        }

        // Activate on-hold clients on first actual connection
        if let Ok(Some(c)) = client::Entity::find_by_id(client_id).one(&state.db).await {
            if c.status == "on_hold" {
                let now = chrono::Utc::now();
                let expire_date = c
                    .on_hold_expire_duration_secs
                    .map(|secs| now + chrono::Duration::seconds(secs));

                let mut active: client::ActiveModel = c.into();
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
        }

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
            last_active_at: Set(chrono::Utc::now().into()),
        };
        active.insert(&state.db).await?;
    }

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
