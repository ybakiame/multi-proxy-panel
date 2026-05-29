use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::node_binding;
use sea_orm::{ActiveModelTrait, EntityTrait, QueryFilter, Set};
use sea_orm::ColumnTrait;
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

pub async fn list_bindings(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let mut query = node_binding::Entity::find();

    if let Some(node_id) = params.get("node_id") {
        if let Ok(id) = Uuid::parse_str(node_id) {
            query = query.filter(node_binding::Column::NodeId.eq(id));
        }
    }
    if let Some(config_id) = params.get("config_id") {
        if let Ok(id) = Uuid::parse_str(config_id) {
            query = query.filter(node_binding::Column::ProtocolConfigId.eq(id));
        }
    }

    let bindings = query
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<Value> = bindings.into_iter().map(|b| json!({
        "id": b.id,
        "node_id": b.node_id,
        "protocol_config_id": b.protocol_config_id,
        "is_active": b.is_active,
        "override_settings": b.override_settings,
    })).collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_binding(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let node_id = payload
        .get("node_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let protocol_config_id = payload
        .get("protocol_config_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let active = node_binding::ActiveModel {
        id: Set(Uuid::new_v4()),
        node_id: Set(node_id),
        protocol_config_id: Set(protocol_config_id),
        override_settings: Set(payload.get("override_settings").cloned()),
        is_active: Set(payload.get("is_active").and_then(|v| v.as_bool()).unwrap_or(true)),
        created_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": inserted.id,
        "node_id": inserted.node_id,
        "protocol_config_id": inserted.protocol_config_id,
        "is_active": inserted.is_active,
    } })))
}

pub async fn delete_binding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = node_binding::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}
