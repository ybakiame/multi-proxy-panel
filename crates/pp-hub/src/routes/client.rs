use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::{client, client_group_binding, node_user_usage_record};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
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
    client_group_binding::Entity::delete_many()
        .filter(client_group_binding::Column::ClientId.eq(client_id))
        .exec(db)
        .await?;

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

/// Calculate total traffic used by a client from node_user_usage_records.
async fn get_client_traffic_total(
    db: &sea_orm::DatabaseConnection,
    client_id: Uuid,
) -> Result<i64, sea_orm::DbErr> {
    let records = node_user_usage_record::Entity::find()
        .filter(node_user_usage_record::Column::ClientId.eq(client_id))
        .all(db)
        .await?;
    Ok(records
        .iter()
        .map(|r| r.upload_bytes + r.download_bytes)
        .sum())
}

fn client_to_json(c: client::Model, group_ids: Vec<Uuid>, traffic_total: i64) -> Value {
    let is_exceeded = c.traffic_limit_bytes > 0 && traffic_total >= c.traffic_limit_bytes;
    json!({
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
        "on_hold_expire_duration_secs": c.on_hold_expire_duration_secs,
        "on_hold_timeout": c.on_hold_timeout,
        "group_ids": group_ids,
    })
}

pub async fn list_clients(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (clients, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = client::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = client::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = client::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let mut data = Vec::with_capacity(clients.len());
    for c in clients {
        let group_ids = get_client_group_ids(&state.db, c.id)
            .await
            .map_err(ApiError::from)?;
        let traffic_total = get_client_traffic_total(&state.db, c.id)
            .await
            .map_err(ApiError::from)?;
        data.push(client_to_json(c, group_ids, traffic_total));
    }

    Ok(PaginatedResponse::new(data, total))
}

pub async fn get_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let c = client::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("client not found"))?;

    let group_ids = get_client_group_ids(&state.db, c.id)
        .await
        .map_err(ApiError::from)?;
    let traffic_total = get_client_traffic_total(&state.db, c.id)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiResponse::new(client_to_json(
        c,
        group_ids,
        traffic_total,
    )))
}

#[derive(serde::Deserialize)]
pub struct CreateClientPayload {
    pub name: String,
    pub user_id: Option<Uuid>,
    pub email: Option<String>,
    pub traffic_limit_bytes: Option<i64>,
    pub expiry_date: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub reset_day: Option<i32>,
    pub data_limit_reset_strategy: Option<String>,
    pub max_devices: Option<i32>,
    pub group_ids: Option<Vec<Uuid>>,
    pub on_hold_expire_duration_secs: Option<i64>,
    pub on_hold_timeout: Option<chrono::DateTime<chrono::FixedOffset>>,
}

pub async fn create_client(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateClientPayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "client name is required",
        ));
    }

    // Determine status: on_hold if on_hold_expire_duration is set and no expiry_date
    let is_on_hold =
        payload.on_hold_expire_duration_secs.is_some() && payload.expiry_date.is_none();
    let status = if is_on_hold { "on_hold" } else { "active" };

    let active = client::ActiveModel {
        id: Set(Uuid::new_v4()),
        user_id: Set(payload.user_id.unwrap_or_else(Uuid::new_v4)),
        name: Set(payload.name.trim().to_string()),
        email: Set(payload.email.filter(|e| !e.is_empty())),
        traffic_limit_bytes: Set(payload.traffic_limit_bytes.unwrap_or(0)),
        traffic_used_bytes: Set(0),
        all_time_used_bytes: Set(0),
        expiry_date: Set(payload.expiry_date),
        reset_day: Set(payload.reset_day),
        data_limit_reset_strategy: Set(payload
            .data_limit_reset_strategy
            .unwrap_or_else(|| "no_reset".to_string())),
        last_traffic_reset_time: Set(None),
        max_devices: Set(payload.max_devices),
        on_hold_expire_duration_secs: Set(payload.on_hold_expire_duration_secs),
        on_hold_timeout: Set(payload.on_hold_timeout),
        status: Set(status.to_string()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    if let Some(group_ids) = payload.group_ids {
        sync_client_groups(&state.db, inserted.id, &group_ids)
            .await
            .map_err(ApiError::from)?;
    }

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "name": inserted.name,
        "email": inserted.email,
        "status": inserted.status,
    })))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateClientPayload {
    pub name: Option<String>,
    pub email: Option<String>,
    pub traffic_limit_bytes: Option<i64>,
    pub expiry_date: Option<chrono::DateTime<chrono::FixedOffset>>,
    pub reset_day: Option<i32>,
    pub data_limit_reset_strategy: Option<String>,
    pub max_devices: Option<i32>,
    pub status: Option<String>,
    pub group_ids: Option<Vec<Uuid>>,
    pub on_hold_expire_duration_secs: Option<i64>,
    pub on_hold_timeout: Option<chrono::DateTime<chrono::FixedOffset>>,
}

pub async fn update_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateClientPayload>,
) -> ApiResult<Value> {
    let c = client::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("client not found"))?;

    let mut active: client::ActiveModel = c.into();

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "client name cannot be empty",
            ));
        }
        active.name = Set(name.trim().to_string());
    }
    if let Some(email) = payload.email {
        active.email = Set(Some(email).filter(|e| !e.is_empty()));
    }
    if let Some(limit) = payload.traffic_limit_bytes {
        active.traffic_limit_bytes = Set(limit);
    }
    if payload.expiry_date.is_some() {
        active.expiry_date = Set(payload.expiry_date);
    }
    if let Some(day) = payload.reset_day {
        active.reset_day = Set(Some(day));
    }
    if let Some(strategy) = payload.data_limit_reset_strategy {
        active.data_limit_reset_strategy = Set(strategy);
    }
    if let Some(max) = payload.max_devices {
        active.max_devices = Set(Some(max));
    }
    if let Some(status) = payload.status {
        active.status = Set(status);
    }
    if payload.on_hold_expire_duration_secs.is_some() {
        active.on_hold_expire_duration_secs = Set(payload.on_hold_expire_duration_secs);
    }
    if payload.on_hold_timeout.is_some() {
        active.on_hold_timeout = Set(payload.on_hold_timeout);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    if let Some(group_ids) = payload.group_ids {
        sync_client_groups(&state.db, updated.id, &group_ids)
            .await
            .map_err(ApiError::from)?;
    }

    Ok(ApiResponse::new(json!({
        "id": updated.id,
        "name": updated.name,
        "status": updated.status,
    })))
}

pub async fn delete_client(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = client::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("client not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

#[derive(serde::Deserialize)]
pub struct ClientResetTrafficPayload {
    #[allow(dead_code)]
    pub reason: Option<String>,
}

pub async fn reset_client_traffic(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(_payload): Json<ClientResetTrafficPayload>,
) -> ApiResult<Value> {
    let c = client::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("client not found"))?;

    let mut active: client::ActiveModel = c.into();
    active.traffic_used_bytes = Set(0);
    active.last_traffic_reset_time = Set(Some(chrono::Utc::now().into()));
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(json!({
        "id": updated.id,
        "traffic_used_bytes": updated.traffic_used_bytes,
        "last_traffic_reset_time": updated.last_traffic_reset_time,
    })))
}
