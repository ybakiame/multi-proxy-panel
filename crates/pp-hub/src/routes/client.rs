use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::{client, client_group_binding, traffic_record};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

/// Fetch group IDs assigned to a client.
async fn get_client_group_ids(
    db: &sea_orm::DatabaseConnection,
    client_id: Uuid,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let bindings = client_group_binding::Entity::find()
        .filter(client_group_binding::Column::ClientId.eq(client_id))
        .all(db)
        .await?;
    Ok(bindings.into_iter().map(|b| b.group_id).collect())
}

/// Replace a client's group bindings with the provided list.
async fn sync_client_groups(
    db: &sea_orm::DatabaseConnection,
    client_id: Uuid,
    group_ids: &[Uuid],
) -> Result<(), sea_orm::DbErr> {
    // Delete existing bindings
    client_group_binding::Entity::delete_many()
        .filter(client_group_binding::Column::ClientId.eq(client_id))
        .exec(db)
        .await?;

    // Insert new bindings
    for &group_id in group_ids {
        let binding = client_group_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            client_id: Set(client_id),
            group_id: Set(group_id),
            created_at: Set(chrono::Utc::now().into()),
        };
        binding.insert(db).await?;
    }

    Ok(())
}

/// Calculate total traffic used by a client from traffic_records.
async fn get_client_traffic_total(
    db: &sea_orm::DatabaseConnection,
    client_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let records = traffic_record::Entity::find()
        .filter(traffic_record::Column::ClientId.eq(client_id))
        .all(db)
        .await?;
    let total: i64 = records
        .iter()
        .map(|r| r.upload_bytes + r.download_bytes)
        .sum();
    Ok(total)
}

pub async fn list_clients(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let clients = client::Entity::find()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut data = Vec::with_capacity(clients.len());
    for c in clients {
        let group_ids = get_client_group_ids(&state.db, c.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let traffic_total = get_client_traffic_total(&state.db, c.id)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let is_exceeded = c.traffic_limit_bytes > 0 && traffic_total >= c.traffic_limit_bytes;
        data.push(json!({
            "id": c.id,
            "user_id": c.user_id,
            "name": c.name,
            "email": c.email,
            "traffic_limit_bytes": c.traffic_limit_bytes,
            "traffic_used_bytes": c.traffic_used_bytes,
            "all_time_used_bytes": c.all_time_used_bytes,
            "traffic_used_total": traffic_total,
            "is_exceeded": is_exceeded,
            "expiry_date": c.expiry_date,
            "reset_day": c.reset_day,
            "data_limit_reset_strategy": c.data_limit_reset_strategy,
            "last_traffic_reset_time": c.last_traffic_reset_time,
            "max_devices": c.max_devices,
            "status": c.status,
            "group_ids": group_ids,
        }));
    }

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

    let group_ids = get_client_group_ids(&state.db, c.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let traffic_total = get_client_traffic_total(&state.db, c.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let is_exceeded = c.traffic_limit_bytes > 0 && traffic_total >= c.traffic_limit_bytes;

    Ok(Json(json!({ "data": {
        "id": c.id,
        "user_id": c.user_id,
        "name": c.name,
        "email": c.email,
        "traffic_limit_bytes": c.traffic_limit_bytes,
        "traffic_used_bytes": c.traffic_used_bytes,
        "all_time_used_bytes": c.all_time_used_bytes,
        "traffic_used_total": traffic_total,
        "is_exceeded": is_exceeded,
        "expiry_date": c.expiry_date,
        "reset_day": c.reset_day,
        "data_limit_reset_strategy": c.data_limit_reset_strategy,
        "last_traffic_reset_time": c.last_traffic_reset_time,
        "status": c.status,
        "group_ids": group_ids,
    } })))
}

pub async fn create_client(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let active = client::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(payload
            .get("user_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(Uuid::new_v4)),
        name: Set(name.to_string()),
        email: Set(payload
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())),
        traffic_limit_bytes: Set(payload
            .get("traffic_limit_bytes")
            .and_then(|v| v.as_i64())
            .unwrap_or(0)),
        traffic_used_bytes: Set(0),
        all_time_used_bytes: Set(0),
        expiry_date: Set(None),
        reset_day: Set(payload
            .get("reset_day")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)),
        data_limit_reset_strategy: Set(payload
            .get("data_limit_reset_strategy")
            .and_then(|v| v.as_str())
            .unwrap_or("no_reset")
            .to_string()),
        last_traffic_reset_time: Set(None),
        max_devices: Set(payload
            .get("max_devices")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)),
        status: Set("active".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Sync group bindings if provided
    if let Some(group_ids) = parse_group_ids(&payload) {
        sync_client_groups(&state.db, inserted.id, &group_ids)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

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
    let c = client::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let mut active: client::ActiveModel = c.into();

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        active.name = Set(name.to_string());
    }
    if payload.get("email").is_some() {
        active.email = Set(payload
            .get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()));
    }
    if let Some(limit) = payload.get("traffic_limit_bytes").and_then(|v| v.as_i64()) {
        active.traffic_limit_bytes = Set(limit);
    }
    if let Some(strategy) = payload.get("data_limit_reset_strategy").and_then(|v| v.as_str()) {
        active.data_limit_reset_strategy = Set(strategy.to_string());
    }
    if payload.get("max_devices").is_some() {
        active.max_devices = Set(payload
            .get("max_devices")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32));
    }
    if let Some(status) = payload.get("status").and_then(|v| v.as_str()) {
        active.status = Set(status.to_string());
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active
        .update(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Sync group bindings if provided
    if payload.get("group_ids").is_some() {
        if let Some(group_ids) = parse_group_ids(&payload) {
            sync_client_groups(&state.db, updated.id, &group_ids)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        } else {
            // group_ids is explicitly null/empty — clear all bindings
            client_group_binding::Entity::delete_many()
                .filter(client_group_binding::Column::ClientId.eq(updated.id))
                .exec(&state.db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }
    }

    Ok(Json(
        json!({ "data": { "id": updated.id, "name": updated.name, "status": updated.status } }),
    ))
}

pub async fn delete_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    // Clean up group bindings first
    let _ = client_group_binding::Entity::delete_many()
        .filter(client_group_binding::Column::ClientId.eq(id))
        .exec(&state.db)
        .await;

    let res = client::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

/// Parse a JSON array of group IDs from the payload.
fn parse_group_ids(payload: &Value) -> Option<Vec<Uuid>> {
    payload
        .get("group_ids")?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
        .collect::<Vec<_>>()
        .into()
}
