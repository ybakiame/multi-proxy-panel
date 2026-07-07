use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::webhook;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QuerySelect, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn webhook_to_json(h: webhook::Model) -> Value {
    json!({
        "id": h.id,
        "name": h.name,
        "url": h.url,
        "events": h.events,
        "is_active": h.is_active,
        "created_at": h.created_at,
    })
}

pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (hooks, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = webhook::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = webhook::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = webhook::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = hooks.into_iter().map(webhook_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateWebhookPayload {
    pub name: String,
    pub url: String,
    pub events: Option<Value>,
    pub secret: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateWebhookPayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "webhook name is required",
        ));
    }
    if payload.url.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_url",
            "webhook URL is required",
        ));
    }

    let active = webhook::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        url: Set(payload.url),
        events: Set(payload.events.unwrap_or_else(|| json!([]))),
        secret: Set(payload.secret.filter(|s| !s.is_empty())),
        is_active: Set(payload.is_active.unwrap_or(true)),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(webhook_to_json(inserted)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateWebhookPayload {
    pub name: Option<String>,
    pub url: Option<String>,
    pub events: Option<Value>,
    pub secret: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn update_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateWebhookPayload>,
) -> ApiResult<Value> {
    let h = webhook::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("webhook not found"))?;

    let mut active: webhook::ActiveModel = h.into();

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "webhook name cannot be empty",
            ));
        }
        active.name = Set(name);
    }
    if let Some(url) = payload.url {
        if url.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_url",
                "webhook URL cannot be empty",
            ));
        }
        active.url = Set(url);
    }
    if let Some(events) = payload.events {
        active.events = Set(events);
    }
    if payload.secret.is_some() {
        active.secret = Set(payload.secret.filter(|s| !s.is_empty()));
    }
    if let Some(active_flag) = payload.is_active {
        active.is_active = Set(active_flag);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(webhook_to_json(updated)))
}

pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = webhook::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("webhook not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}
