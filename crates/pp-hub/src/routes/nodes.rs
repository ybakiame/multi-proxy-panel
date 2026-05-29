use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_common::{NodeDto, NodeStatus};
use pp_db::entities::node;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

pub async fn list_nodes(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let nodes = node::Entity::find()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let dtos: Vec<NodeDto> = nodes
        .into_iter()
        .map(|n| NodeDto {
            id: n.id,
            name: n.name,
            hostname: n.hostname,
            address: n.address,
            cores_available: serde_json::from_value(n.cores_available).unwrap_or_default(),
            labels: n.labels.unwrap_or(json!({})),
            status: match n.status.as_str() {
                "online" => NodeStatus::Online,
                "offline" => NodeStatus::Offline,
                "degraded" => NodeStatus::Degraded,
                _ => NodeStatus::Connecting,
            },
            last_seen_at: n.last_seen_at.map(|dt| dt.with_timezone(&chrono::Utc)),
        })
        .collect();

    Ok(Json(json!({ "data": dtos })))
}

pub async fn get_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let node = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "data": {
            "id": node.id,
            "name": node.name,
            "hostname": node.hostname,
            "address": node.address,
            "status": node.status,
        }
    })))
}

pub async fn create_node(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let active = node::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        hostname: Set(payload.get("hostname").and_then(|v| v.as_str()).unwrap_or("").to_string()),
        address: Set(payload.get("address").and_then(|v| v.as_str()).unwrap_or("").to_string()),
        token_hash: Set("".to_string()), // TODO: generate and hash token
        cores_available: Set(json!([])),
        labels: Set(None),
        status: Set("connecting".to_string()),
        last_seen_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "data": {
            "id": inserted.id,
            "name": inserted.name,
            "hostname": inserted.hostname,
            "address": inserted.address,
            "status": inserted.status,
        }
    })))
}

pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = node::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

pub async fn push_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let target_core = payload
        .get("core_type")
        .and_then(|v| v.as_str())
        .unwrap_or("sing-box");

    let core_type = match target_core {
        "xray" => pp_common::CoreType::Xray,
        "sing-box" | "singbox" => pp_common::CoreType::SingBox,
        _ => pp_common::CoreType::SingBox,
    };

    // Generate config from database bindings
    let config_json = crate::service::protocol::generate_node_config(&state.db, id, core_type)
        .await
        .map_err(|e| {
            tracing::warn!("failed to generate config for node {}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let config_str = serde_json::to_string(&config_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let proto_core = match core_type {
        pp_common::CoreType::Xray => pp_proto::CoreType::Xray,
        pp_common::CoreType::SingBox => pp_proto::CoreType::SingBox,
        pp_common::CoreType::Both => pp_proto::CoreType::Both,
    };

    let message = pp_proto::HubMessage {
        payload: Some(pp_proto::hub_message::Payload::ConfigPush(
            pp_proto::ConfigPush {
                config_json: config_str,
                target_core: proto_core as i32,
                restart_required: payload.get("restart").and_then(|v| v.as_bool()).unwrap_or(true),
                config_version: payload
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1")
                    .to_string(),
            },
        )),
    };

    state
        .send_to_agent(id, message)
        .await
        .map_err(|e| {
            tracing::warn!("failed to push config to agent {}: {}", id, e);
            StatusCode::BAD_GATEWAY
        })?;

    Ok(Json(json!({ "success": true, "message": "config pushed" })))
}
