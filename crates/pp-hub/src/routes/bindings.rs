use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::{node, node_binding, node_binding_group_binding, protocol_config};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

async fn get_binding_group_ids(
    db: &sea_orm::DatabaseConnection,
    binding_id: Uuid,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let bindings = node_binding_group_binding::Entity::find()
        .filter(node_binding_group_binding::Column::NodeBindingId.eq(binding_id))
        .all(db)
        .await?;
    Ok(bindings.into_iter().map(|b| b.group_id).collect())
}

fn binding_to_json(b: node_binding::Model, group_ids: Vec<Uuid>) -> Value {
    json!({
        "id": b.id,
        "node_id": b.node_id,
        "protocol_config_id": b.protocol_config_id,
        "is_active": b.is_active,
        "override_settings": b.override_settings,
        "group_ids": group_ids,
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

    let mut data = Vec::with_capacity(bindings.len());
    for b in bindings {
        let group_ids = get_binding_group_ids(&state.db, b.id)
            .await
            .map_err(ApiError::from)?;
        data.push(binding_to_json(b, group_ids));
    }
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

    Ok(ApiResponse::new(binding_to_json(inserted, Vec::new())))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateBindingPayload {
    pub is_active: Option<bool>,
    pub override_settings: Option<Value>,
}

pub async fn update_binding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateBindingPayload>,
) -> ApiResult<Value> {
    let binding = node_binding::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("binding not found"))?;

    let mut active: node_binding::ActiveModel = binding.into();

    if let Some(is_active) = payload.is_active {
        active.is_active = Set(is_active);
    }
    if payload.override_settings.is_some() {
        active.override_settings = Set(payload.override_settings);
    }

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;
    let group_ids = get_binding_group_ids(&state.db, updated.id)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiResponse::new(binding_to_json(updated, group_ids)))
}

pub async fn delete_binding(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let _ = node_binding_group_binding::Entity::delete_many()
        .filter(node_binding_group_binding::Column::NodeBindingId.eq(id))
        .exec(&state.db)
        .await;

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
