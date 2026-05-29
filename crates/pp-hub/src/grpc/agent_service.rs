use pp_db::entities::{host_metric, node, system_log};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use pp_proto::{
    hub_agent_server::HubAgent, AgentMessage, CoreCommand, CoreStart, CoreStatusReport,
    CoreStop, CoreType, Heartbeat, HostMetrics, HubMessage, LogBatch, RegisterRequest,
    RegisterResponse, TrafficReport,
};
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
}

#[tonic::async_trait]
impl HubAgent for HubAgentService {
    type StreamStream =
        Pin<Box<dyn Stream<Item = Result<HubMessage, Status>> + Send + 'static>>;

    async fn stream(
        &self,
        request: Request<Streaming<AgentMessage>>,
    ) -> Result<Response<Self::StreamStream>, Status> {
        let mut inbound = request.into_inner();
        let state = self.state.clone();

        // (tx, rx) for sending messages from Hub to Agent
        let (tx, mut rx) = mpsc::channel::<HubMessage>(128);
        let (registered_tx, registered_rx) = tokio::sync::oneshot::channel::<Uuid>();

        // Spawn a task to handle inbound messages from this Agent
        let inbound_handle = tokio::spawn(async move {
            let mut registered_id: Option<Uuid> = None;

            while let Some(msg_result) = inbound.next().await {
                match msg_result {
                    Ok(msg) => {
                        if let Err(e) =
                            handle_agent_message(&state, msg, &mut registered_id, &tx).await
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

        Ok(Response::new(Box::pin(outbound_stream) as Self::StreamStream))
    }
}

async fn handle_agent_message(
    state: &AppState,
    msg: AgentMessage,
    registered_id: &mut Option<Uuid>,
    hub_tx: &mpsc::Sender<HubMessage>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use pp_proto::agent_message::Payload;

    match msg.payload {
        Some(Payload::Register(req)) => {
            let agent_id = handle_register(state, req, hub_tx).await?;
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
) -> Result<Uuid, Box<dyn std::error::Error + Send + Sync>> {
    let agent_id = Uuid::parse_str(&req.agent_id)?;

    // Verify token against database
    let node = node::Entity::find()
        .filter(node::Column::Id.eq(agent_id))
        .one(&state.db)
        .await?;

    let node_exists = node.is_some();

    // If node doesn't exist, auto-create it (auto-register mode)
    if !node_exists {
        let new_node = node::ActiveModel {
            id: Set(agent_id),
            name: Set(req.hostname.clone()),
            hostname: Set(req.hostname.clone()),
            address: Set("".to_string()),
            token_hash: Set(req.token.clone()), // TODO: hash the token
            cores_available: Set(serde_json::json!(req.capabilities)),
            labels: Set(Some(serde_json::json!(
                req.labels.into_iter().collect::<std::collections::HashMap<_, _>>()
            ))),
            status: Set("online".to_string()),
            last_seen_at: Set(Some(chrono::Utc::now().into())),
            created_at: Set(chrono::Utc::now().into()),
            updated_at: Set(chrono::Utc::now().into()),
        };
        new_node.insert(&state.db).await?;
        tracing::info!("auto-registered new agent: {}", agent_id);
    } else {
        // Update last_seen_at and status
        let node = node.unwrap();
        let mut active: node::ActiveModel = node.into();
        active.status = Set("online".to_string());
        active.last_seen_at = Set(Some(chrono::Utc::now().into()));
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(&state.db).await?;
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
    hub_tx.send(resp).await?;

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
    _state: &AppState,
    agent_id: Uuid,
    traffic: TrafficReport,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::debug!(
        "traffic report from {}: {} inbounds, {} users",
        agent_id,
        traffic.inbounds.len(),
        traffic.users.len()
    );
    // TODO: aggregate and persist to traffic_records
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
