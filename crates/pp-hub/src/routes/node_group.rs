use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::{node_binding_group_binding, node_group};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn group_to_json(g: node_group::Model) -> Value {
    json!({
        "id": g.id,
        "name": g.name,
        "description": g.description,
        "labels": g.labels.unwrap_or(json!({})),
        "created_at": g.created_at,
        "updated_at": g.updated_at,
    })
}

async fn sync_group_bindings(
    db: &sea_orm::DatabaseConnection,
    group_id: Uuid,
    binding_ids: &[Uuid],
) -> Result<(), sea_orm::DbErr> {
    node_binding_group_binding::Entity::delete_many()
        .filter(node_binding_group_binding::Column::GroupId.eq(group_id))
        .exec(db)
        .await?;

    for &binding_id in binding_ids {
        let binding = node_binding_group_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            group_id: Set(group_id),
            node_binding_id: Set(binding_id),
            created_at: Set(chrono::Utc::now().into()),
        };
        binding.insert(db).await?;
    }

    Ok(())
}

pub async fn list_groups(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (groups, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = node_group::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = node_group::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = node_group::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = groups.into_iter().map(group_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

pub async fn get_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let g = node_group::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node group not found"))?;

    Ok(ApiResponse::new(group_to_json(g)))
}

#[derive(serde::Deserialize)]
pub struct CreateGroupPayload {
    pub name: String,
    pub description: Option<String>,
    pub labels: Option<Value>,
    pub binding_ids: Option<Vec<Uuid>>,
}

pub async fn create_group(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateGroupPayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "group name is required",
        ));
    }

    let active = node_group::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        description: Set(payload.description.filter(|s| !s.is_empty())),
        labels: Set(payload.labels),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    if let Some(binding_ids) = payload.binding_ids {
        sync_group_bindings(&state.db, inserted.id, &binding_ids)
            .await
            .map_err(ApiError::from)?;
    }

    Ok(ApiResponse::new(group_to_json(inserted)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateGroupPayload {
    pub name: Option<String>,
    pub description: Option<String>,
    pub labels: Option<Value>,
    pub binding_ids: Option<Vec<Uuid>>,
}

pub async fn update_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateGroupPayload>,
) -> ApiResult<Value> {
    let g = node_group::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node group not found"))?;

    let mut active: node_group::ActiveModel = g.into();

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "group name cannot be empty",
            ));
        }
        active.name = Set(name);
    }
    active.description = Set(payload.description);
    if payload.labels.is_some() {
        active.labels = Set(payload.labels);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    if payload.binding_ids.is_some() {
        let binding_ids = payload.binding_ids.unwrap_or_default();
        sync_group_bindings(&state.db, updated.id, &binding_ids)
            .await
            .map_err(ApiError::from)?;
    }

    Ok(ApiResponse::new(group_to_json(updated)))
}

pub async fn delete_group(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = node_group::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("node group not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}
