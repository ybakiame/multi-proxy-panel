use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::webhook;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

pub async fn list_webhooks(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, StatusCode> {
    let hooks = webhook::Entity::find()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<Value> = hooks
        .into_iter()
        .map(|h| {
            json!({
                "id": h.id,
                "name": h.name,
                "url": h.url,
                "events": h.events,
                "is_active": h.is_active,
                "created_at": h.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "data": data })))
}

pub async fn create_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;
    let url = payload
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let active = webhook::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        url: Set(url.to_string()),
        events: Set(payload.get("events").cloned().unwrap_or(json!([]))),
        secret: Set(payload.get("secret").and_then(|v| v.as_str()).map(|s| s.to_string())),
        is_active: Set(payload.get("is_active").and_then(|v| v.as_bool()).unwrap_or(true)),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": inserted.id,
        "name": inserted.name,
        "url": inserted.url,
        "events": inserted.events,
        "is_active": inserted.is_active,
    } })))
}

pub async fn update_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let h = webhook::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: webhook::ActiveModel = h.into();

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        active.name = Set(name.to_string());
    }
    if let Some(url) = payload.get("url").and_then(|v| v.as_str()) {
        active.url = Set(url.to_string());
    }
    if payload.get("events").is_some() {
        active.events = Set(payload.get("events").cloned().unwrap_or(json!([])));
    }
    if payload.get("secret").is_some() {
        active.secret = Set(payload.get("secret").and_then(|v| v.as_str()).map(|s| s.to_string()));
    }
    if let Some(active_flag) = payload.get("is_active").and_then(|v| v.as_bool()) {
        active.is_active = Set(active_flag);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active
        .update(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": updated.id,
        "name": updated.name,
        "url": updated.url,
        "events": updated.events,
        "is_active": updated.is_active,
    } })))
}

pub async fn delete_webhook(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = webhook::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}
