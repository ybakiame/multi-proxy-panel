use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::{node, node_group_binding};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
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

fn node_to_json(n: node::Model, group_ids: Vec<Uuid>) -> Value {
    json!({
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
    })
}

pub async fn list_nodes(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (nodes, total) = if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
        let total = node::Entity::find()
            .count(&state.db)
            .await
            .map_err(ApiError::from)? as u64;
        let items = node::Entity::find()
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        (items, total)
    } else {
        let items = node::Entity::find()
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        let total = items.len() as u64;
        (items, total)
    };

    let mut data = Vec::with_capacity(nodes.len());
    for n in nodes {
        let group_ids = get_node_group_ids(&state.db, n.id)
            .await
            .map_err(ApiError::from)?;
        data.push(node_to_json(n, group_ids));
    }

    Ok(PaginatedResponse::new(data, total))
}

pub async fn get_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let n = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node not found"))?;

    let group_ids = get_node_group_ids(&state.db, n.id)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiResponse::new(node_to_json(n, group_ids)))
}

#[derive(serde::Deserialize)]
pub struct CreateNodePayload {
    pub name: String,
    pub hostname: Option<String>,
    pub address: Option<String>,
    pub group_ids: Option<Vec<Uuid>>,
}

pub async fn create_node(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateNodePayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request("invalid_name", "node name is required"));
    }

    let raw_token = pp_common::generate_secure_token();
    let token_hash = pp_common::hash_secret(&raw_token)
        .map_err(|e| ApiError::internal(format!("failed to hash token: {e}")))?;

    let active = node::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        hostname: Set(payload.hostname.unwrap_or_default()),
        address: Set(payload.address.unwrap_or_default()),
        token_hash: Set(token_hash),
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
        .map_err(ApiError::from)?;

    if let Some(group_ids) = payload.group_ids {
        sync_node_groups(&state.db, inserted.id, &group_ids)
            .await
            .map_err(ApiError::from)?;
    }

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "name": inserted.name,
        "hostname": inserted.hostname,
        "address": inserted.address,
        "status": inserted.status,
        "token": raw_token,
    })))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateNodePayload {
    pub name: Option<String>,
    pub hostname: Option<String>,
    pub address: Option<String>,
    pub usage_coefficient: Option<f32>,
    pub labels: Option<Value>,
    pub group_ids: Option<Vec<Uuid>>,
}

pub async fn update_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateNodePayload>,
) -> ApiResult<Value> {
    let n = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node not found"))?;

    let mut active: node::ActiveModel = n.into();

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request("invalid_name", "node name cannot be empty"));
        }
        active.name = Set(name);
    }
    if let Some(hostname) = payload.hostname {
        active.hostname = Set(hostname);
    }
    if let Some(address) = payload.address {
        active.address = Set(address);
    }
    if let Some(coefficient) = payload.usage_coefficient {
        active.usage_coefficient = Set(coefficient);
    }
    if payload.labels.is_some() {
        active.labels = Set(payload.labels);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active
        .update(&state.db)
        .await
        .map_err(ApiError::from)?;

    if payload.group_ids.is_some() {
        let group_ids = payload.group_ids.unwrap_or_default();
        sync_node_groups(&state.db, updated.id, &group_ids)
            .await
            .map_err(ApiError::from)?;
    }

    let group_ids = get_node_group_ids(&state.db, updated.id)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiResponse::new(node_to_json(updated, group_ids)))
}

pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let _ = node_group_binding::Entity::delete_many()
        .filter(node_group_binding::Column::NodeId.eq(id))
        .exec(&state.db)
        .await;

    let res = node::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("node not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

#[derive(serde::Deserialize)]
pub struct PushConfigPayload {
    pub core_type: Option<String>,
    pub restart: Option<bool>,
    pub version: Option<String>,
}

pub async fn push_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<PushConfigPayload>,
) -> ApiResult<Value> {
    let target_core = payload.core_type.as_deref().unwrap_or("sing-box");

    let core_type = match target_core {
        "xray" => pp_common::CoreType::Xray,
        "sing-box" | "singbox" => pp_common::CoreType::SingBox,
        _ => {
            return Err(ApiError::bad_request(
                "invalid_core_type",
                "core_type must be 'xray' or 'sing-box'",
            ));
        }
    };

    crate::service::protocol::validate_node_port_conflicts(&state.db, id)
        .await
        .map_err(|e| {
            tracing::warn!("node {} port conflict: {}", id, e);
            ApiError::new(axum::http::StatusCode::CONFLICT, "port_conflict", e.to_string())
        })?;

    let config_json = crate::service::protocol::generate_node_config(&state.db, id, core_type)
        .await
        .map_err(|e| {
            tracing::warn!("failed to generate config for node {}: {}", id, e);
            ApiError::internal(format!("failed to generate config: {e}"))
        })?;

    let config_str = serde_json::to_string(&config_json).map_err(|e| {
        ApiError::internal(format!("failed to serialize config: {e}"))
    })?;

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
                restart_required: payload.restart.unwrap_or(true),
                config_version: payload.version.unwrap_or_else(|| "1".to_string()),
            },
        )),
    };

    state.send_to_agent(id, message).await.map_err(|e| {
        tracing::warn!("failed to push config to agent {}: {}", id, e);
        ApiError::new(axum::http::StatusCode::BAD_GATEWAY, "agent_unreachable", e.to_string())
    })?;

    Ok(ApiResponse::new(json!({
        "success": true,
        "message": "config pushed",
    })))
}
