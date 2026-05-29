use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::client;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

pub async fn list_clients(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let clients = client::Entity::find()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<Value> = clients.into_iter().map(|c| json!({
        "id": c.id,
        "user_id": c.user_id,
        "name": c.name,
        "email": c.email,
        "traffic_limit_bytes": c.traffic_limit_bytes,
        "traffic_used_bytes": c.traffic_used_bytes,
        "expiry_date": c.expiry_date,
        "reset_day": c.reset_day,
        "status": c.status,
    })).collect();

    Ok(Json(json!({ "data": data })))
}

pub async fn get_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let c = client::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({ "data": {
        "id": c.id,
        "user_id": c.user_id,
        "name": c.name,
        "email": c.email,
        "traffic_limit_bytes": c.traffic_limit_bytes,
        "traffic_used_bytes": c.traffic_used_bytes,
        "expiry_date": c.expiry_date,
        "reset_day": c.reset_day,
        "status": c.status,
    } })))
}

pub async fn create_client(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload.get("name").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;

    let active = client::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(payload.get("user_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok()).unwrap_or_else(Uuid::new_v4)),
        name: Set(name.to_string()),
        email: Set(payload.get("email").and_then(|v| v.as_str()).map(|s| s.to_string())),
        traffic_limit_bytes: Set(payload.get("traffic_limit_bytes").and_then(|v| v.as_i64()).unwrap_or(0)),
        traffic_used_bytes: Set(0),
        expiry_date: Set(None),
        reset_day: Set(payload.get("reset_day").and_then(|v| v.as_i64()).map(|v| v as i32)),
        status: Set("active".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": inserted.id,
        "name": inserted.name,
        "email": inserted.email,
        "status": inserted.status,
    } })))
}

pub async fn update_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let c = client::Entity::find_by_id(id).one(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    let mut active: client::ActiveModel = c.into();

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        active.name = Set(name.to_string());
    }
    if payload.get("email").is_some() {
        active.email = Set(payload.get("email").and_then(|v| v.as_str()).map(|s| s.to_string()));
    }
    if let Some(limit) = payload.get("traffic_limit_bytes").and_then(|v| v.as_i64()) {
        active.traffic_limit_bytes = Set(limit);
    }
    if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
        active.status = Set(status.to_string());
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "data": { "id": updated.id, "name": updated.name, "status": updated.status } })))
}

pub async fn delete_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = client::Entity::delete_by_id(id).exec(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected == 0 { Err(StatusCode::NOT_FOUND) } else { Ok(StatusCode::NO_CONTENT) }
}
