//! Agent 注册处理（自 agent_service.rs 拆出，控制单文件规模）。

use pp_db::entities::node;
use pp_proto::{HubMessage, RegisterRequest, RegisterResponse};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use tokio::sync::mpsc;
use tonic::Status;
use uuid::Uuid;

use crate::state::AppState;

/// Returns true if the address is a loopback address (IPv4 or IPv6).
fn is_loopback_addr(addr: &str) -> bool {
    addr.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

// 返回 Box<Status> 而非 Status：free fn 的签名会被 clippy result_large_err
// 检查（trait impl 方法不会）；内部 `map_err(Status)?` 经 From<T> for Box<T>
// 自动装箱，唯一的手动拆箱点在 stream() 调用处（map_err(|e| *e)）。
pub(super) async fn handle_register(
    state: &AppState,
    req: RegisterRequest,
    hub_tx: &mpsc::Sender<HubMessage>,
    remote_addr: Option<String>,
) -> Result<Uuid, Box<Status>> {
    let agent_id = Uuid::parse_str(&req.agent_id)
        .map_err(|e| Status::invalid_argument(format!("invalid agent_id: {}", e)))?;

    let node = node::Entity::find()
        .filter(node::Column::Id.eq(agent_id))
        .one(&state.db)
        .await
        .map_err(|e| Status::internal(format!("database error: {}", e)))?;

    let Some(node) = node else {
        tracing::warn!(
            "agent {} attempted to register but node does not exist",
            agent_id
        );
        return Err(Box::new(Status::not_found(
            "node not registered; create the node and provision a token first",
        )));
    };

    if node.token_hash.is_empty() {
        tracing::warn!(
            "node {} has empty token_hash; reject registration",
            agent_id
        );
        return Err(Box::new(Status::failed_precondition(
            "node token is not set; provision a token first",
        )));
    }
    let token_valid = pp_common::verify_secret_async(req.token.clone(), node.token_hash.clone())
        .await
        .unwrap_or(false);
    if !token_valid {
        tracing::warn!("agent {} provided invalid token", agent_id);
        return Err(Box::new(Status::unauthenticated("invalid agent token")));
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
