use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use pp_db::entities::{agent_log, node};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set,
};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn core_status_from_log(log: &agent_log::Model) -> Option<Value> {
    let fields = log.fields.as_ref()?;
    let core_type = fields.get("core_type")?.as_str()?;
    let version = fields.get("version")?.as_str()?;
    let running = fields.get("running")?.as_bool()?;
    let uptime_sec = fields.get("uptime_sec")?.as_u64()?;
    let last_error = fields.get("last_error")?.as_str().unwrap_or("");
    Some(json!({
        "core_type": core_type,
        "version": version,
        "running": running,
        "uptime_sec": uptime_sec,
        "last_error": last_error,
        "updated_at": log.created_at,
    }))
}

async fn latest_core_statuses(
    state: &AppState,
    node_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<Value>>, ApiError> {
    if node_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let logs = agent_log::Entity::find()
        .filter(agent_log::Column::NodeId.is_in(node_ids.to_vec()))
        .filter(agent_log::Column::Target.like("core-%"))
        .order_by_desc(agent_log::Column::CreatedAt)
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let mut by_node: std::collections::HashMap<Uuid, std::collections::HashMap<String, Value>> =
        std::collections::HashMap::new();
    for log in logs {
        if let Some(status) = core_status_from_log(&log) {
            let core_type = status
                .get("core_type")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            by_node
                .entry(log.node_id)
                .or_default()
                .entry(core_type)
                .or_insert(status);
        }
    }

    Ok(by_node
        .into_iter()
        .map(|(node_id, statuses)| (node_id, statuses.into_values().collect()))
        .collect())
}

fn node_to_json(n: node::Model, core_statuses: Vec<Value>) -> Value {
    json!({
        "id": n.id,
        "name": n.name,
        "hostname": n.hostname,
        "address": n.address,
        "domain": n.domain,
        "cores_available": n.cores_available,
        "labels": n.labels.unwrap_or(json!({})),
        "usage_coefficient": n.usage_coefficient,
        "status": n.status,
        "parent_id": n.parent_id,
        "last_seen_at": n.last_seen_at,
        "core_statuses": core_statuses,
    })
}

pub async fn list_nodes(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (nodes, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
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

    let node_ids: Vec<Uuid> = nodes.iter().map(|n| n.id).collect();
    let statuses = latest_core_statuses(&state, &node_ids).await?;

    let data: Vec<Value> = nodes
        .into_iter()
        .map(|n| {
            let s = statuses.get(&n.id).cloned().unwrap_or_default();
            node_to_json(n, s)
        })
        .collect();
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

    let statuses = latest_core_statuses(&state, &[id]).await?;
    let core_statuses = statuses.get(&id).cloned().unwrap_or_default();

    Ok(ApiResponse::new(node_to_json(n, core_statuses)))
}

#[derive(serde::Deserialize)]
pub struct CreateNodePayload {
    pub name: String,
    pub hostname: Option<String>,
    pub address: Option<String>,
    pub domain: Option<String>,
    pub parent_id: Option<Uuid>,
}

pub async fn create_node(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateNodePayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "node name is required",
        ));
    }

    let raw_token = pp_common::generate_secure_token();
    let token_hash = pp_common::hash_secret_async(raw_token.clone())
        .await
        .map_err(|e| ApiError::internal(format!("failed to hash token: {e}")))?;

    let active = node::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        hostname: Set(payload.hostname.unwrap_or_default()),
        address: Set(payload.address.unwrap_or_default()),
        domain: Set(payload.domain),
        token_hash: Set(token_hash),
        cores_available: Set(json!([])),
        labels: Set(None),
        usage_coefficient: Set(1.0),
        status: Set("connecting".to_string()),
        parent_id: Set(payload.parent_id),
        last_seen_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

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
    pub domain: Option<String>,
    pub usage_coefficient: Option<f32>,
    pub labels: Option<Value>,
    pub parent_id: Option<Option<Uuid>>,
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
            return Err(ApiError::bad_request(
                "invalid_name",
                "node name cannot be empty",
            ));
        }
        active.name = Set(name);
    }
    if let Some(hostname) = payload.hostname {
        active.hostname = Set(hostname);
    }
    if let Some(address) = payload.address {
        active.address = Set(address);
    }
    if payload.domain.is_some() {
        active.domain = Set(payload.domain);
    }
    if let Some(coefficient) = payload.usage_coefficient {
        active.usage_coefficient = Set(coefficient);
    }
    if payload.labels.is_some() {
        active.labels = Set(payload.labels);
    }
    if let Some(parent_id) = payload.parent_id {
        active.parent_id = Set(parent_id);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    let statuses = latest_core_statuses(&state, &[updated.id]).await?;
    let core_statuses = statuses.get(&updated.id).cloned().unwrap_or_default();

    Ok(ApiResponse::new(node_to_json(updated, core_statuses)))
}

pub async fn delete_node(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
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
        "sing-box" | "singbox" => pp_common::CoreType::SingBox,
        "mihomo" => pp_common::CoreType::Mihomo,
        _ => {
            return Err(ApiError::bad_request(
                "invalid_core_type",
                "core_type must be 'sing-box' or 'mihomo'",
            ));
        }
    };

    crate::service::protocol::push_node_config(
        &state,
        id,
        core_type,
        payload.restart.unwrap_or(true),
        payload.version,
    )
    .await
    .map_err(|e| {
        tracing::warn!("failed to push config to agent {}: {}", id, e);
        ApiError::new(
            axum::http::StatusCode::BAD_GATEWAY,
            "agent_unreachable",
            e.to_string(),
        )
    })?;

    Ok(ApiResponse::new(json!({
        "success": true,
        "message": "config pushed",
    })))
}

pub async fn query_node_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> PaginatedResult<Value> {
    // Verify node exists
    let _ = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node not found"))?;

    let level = params.get("level");
    let mut query = agent_log::Entity::find().filter(agent_log::Column::NodeId.eq(id));

    if let Some(l) = level {
        query = query.filter(agent_log::Column::Level.eq(l));
    }

    let (records, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = query
                .order_by_desc(agent_log::Column::CreatedAt)
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let limit = params
                .get("limit")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(100);
            let items = query
                .order_by_desc(agent_log::Column::CreatedAt)
                .limit(limit)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = records
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "node_id": r.node_id,
                "level": r.level,
                "target": r.target,
                "message": r.message,
                "fields": r.fields,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(PaginatedResponse::new(data, total))
}
