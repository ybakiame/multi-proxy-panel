use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::{node, node_group_binding};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

/// Fetch group IDs assigned to a node.
async fn get_node_group_ids(
    db: &sea_orm::DatabaseConnection,
    node_id: Uuid,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let bindings = node_group_binding::Entity::find()
        .filter(node_group_binding::Column::NodeId.eq(node_id))
        .all(db)
        .await?;
    Ok(bindings.into_iter().map(|b| b.group_id).collect())
}

/// Replace a node's group bindings with the provided list.
async fn sync_node_groups(
    db: &sea_orm::DatabaseConnection,
    node_id: Uuid,
    group_ids: &[Uuid],
) -> Result<(), sea_orm::DbErr> {
    node_group_binding::Entity::delete_many()
        .filter(node_group_binding::Column::NodeId.eq(node_id))
        .exec(db)
        .await?;

    for &group_id in group_ids {
        let binding = node_group_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            node_id: Set(node_id),
            group_id: Set(group_id),
            created_at: Set(chrono::Utc::now().into()),
        };
        binding.insert(db).await?;
    }

    Ok(())
}

pub async fn list_nodes(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let nodes = node::Entity::find()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut data = Vec::with_capacity(nodes.len());
    for n in nodes {
        let group_ids = get_node_group_ids(&state.db, n.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        data.push(json!({
            "id": n.id,
            "name": n.name,
            "hostname": n.hostname,
            "address": n.address,
            "cores_available": n.cores_available,
            "labels": n.labels.unwrap_or(json!({})),
            "usage_coefficient": n.usage_coefficient,
            "status": n.status,
            "group_ids": group_ids,
            "last_seen_at": n.last_seen_at,
        }));
    }

    Ok(Json(json!({ "data": data })))
}

pub async fn get_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let n = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let group_ids = get_node_group_ids(&state.db, n.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": n.id,
        "name": n.name,
        "hostname": n.hostname,
        "address": n.address,
        "usage_coefficient": n.usage_coefficient,
        "status": n.status,
        "group_ids": group_ids,
    } })))
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
        hostname: Set(payload
            .get("hostname")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()),
        address: Set(payload
            .get("address")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()),
        token_hash: Set("".to_string()), // TODO: generate and hash token
        cores_available: Set(json!([])),
        labels: Set(None),
        usage_coefficient: Set(1.0),
        status: Set("connecting".to_string()),
        last_seen_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Sync group bindings if provided
    if let Some(group_ids) = parse_group_ids(&payload) {
        sync_node_groups(&state.db, inserted.id, &group_ids)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

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

pub async fn update_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let n = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: node::ActiveModel = n.into();

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        active.name = Set(name.to_string());
    }
    if let Some(hostname) = payload.get("hostname").and_then(|v| v.as_str()) {
        active.hostname = Set(hostname.to_string());
    }
    if let Some(address) = payload.get("address").and_then(|v| v.as_str()) {
        active.address = Set(address.to_string());
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active
        .update(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Sync group bindings if provided
    if payload.get("group_ids").is_some() {
        if let Some(group_ids) = parse_group_ids(&payload) {
            sync_node_groups(&state.db, updated.id, &group_ids)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        } else {
            node_group_binding::Entity::delete_many()
                .filter(node_group_binding::Column::NodeId.eq(updated.id))
                .exec(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    Ok(Json(json!({ "data": {
        "id": updated.id,
        "name": updated.name,
        "hostname": updated.hostname,
        "address": updated.address,
        "status": updated.status,
    } })))
}

pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let _ = node_group_binding::Entity::delete_many()
        .filter(node_group_binding::Column::NodeId.eq(id))
        .exec(&state.db)
        .await;

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

    let config_json = crate::service::protocol::generate_node_config(&state.db, id, core_type)
        .await
        .map_err(|e| {
            tracing::warn!("failed to generate config for node {}: {}", id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let config_str =
        serde_json::to_string(&config_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
                restart_required: payload
                    .get("restart")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true),
                config_version: payload
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1")
                    .to_string(),
            },
        )),
    };

    state.send_to_agent(id, message).await.map_err(|e| {
        tracing::warn!("failed to push config to agent {}: {}", id, e);
        StatusCode::BAD_GATEWAY
    })?;

    Ok(Json(json!({ "success": true, "message": "config pushed" })))
}

fn parse_group_ids(payload: &Value) -> Option<Vec<Uuid>> {
    payload
        .get("group_ids")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
        .collect::<Vec<_>>()
        .into()
}
