use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::{node, node_binding, protocol_config};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn binding_to_json(b: node_binding::Model) -> Value {
    json!({
        "id": b.id,
        "node_id": b.node_id,
        "protocol_config_id": b.protocol_config_id,
        "is_active": b.is_active,
        "override_settings": b.override_settings,
    })
}

pub async fn list_bindings(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
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

    let (bindings, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = query
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = query.all(&state.db).await.map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = bindings.into_iter().map(binding_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateBindingPayload {
    pub node_id: Uuid,
    pub protocol_config_id: Uuid,
    pub override_settings: Option<Value>,
    pub is_active: Option<bool>,
}

pub async fn create_binding(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateBindingPayload>,
) -> ApiResult<Value> {
    let node_exists = node::Entity::find_by_id(payload.node_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .is_some();
    if !node_exists {
        return Err(ApiError::not_found("node not found"));
    }

    let config_exists = protocol_config::Entity::find_by_id(payload.protocol_config_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .is_some();
    if !config_exists {
        return Err(ApiError::not_found("protocol config not found"));
    }

    let active = node_binding::ActiveModel {
        id: Set(Uuid::new_v4()),
        node_id: Set(payload.node_id),
        protocol_config_id: Set(payload.protocol_config_id),
        override_settings: Set(payload.override_settings),
        is_active: Set(payload.is_active.unwrap_or(true)),
        created_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(binding_to_json(inserted)))
}

pub async fn delete_binding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = node_binding::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("binding not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}
