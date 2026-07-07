use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::inbound_host;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QuerySelect, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn host_to_json(h: inbound_host::Model) -> Value {
    json!({
        "id": h.id,
        "protocol_config_id": h.protocol_config_id,
        "node_id": h.node_id,
        "remark": h.remark,
        "address": h.address,
        "port": h.port,
        "sni": h.sni,
        "host": h.host,
        "path": h.path,
        "security": h.security,
        "alpn": h.alpn,
        "fingerprint": h.fingerprint,
        "is_active": h.is_active,
        "created_at": h.created_at,
        "updated_at": h.updated_at,
    })
}

pub async fn list_hosts(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (hosts, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = inbound_host::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = inbound_host::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = inbound_host::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = hosts.into_iter().map(host_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateHostPayload {
    pub protocol_config_id: Uuid,
    pub node_id: Uuid,
    pub remark: String,
    pub address: String,
    pub port: i32,
    pub sni: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
    pub security: Option<String>,
    pub alpn: Option<String>,
    pub fingerprint: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn create_host(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateHostPayload>,
) -> ApiResult<Value> {
    if payload.remark.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_remark",
            "remark is required",
        ));
    }
    if payload.address.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_address",
            "address is required",
        ));
    }

    let active = inbound_host::ActiveModel {
        id: Set(Uuid::new_v4()),
        protocol_config_id: Set(payload.protocol_config_id),
        node_id: Set(payload.node_id),
        remark: Set(payload.remark),
        address: Set(payload.address),
        port: Set(payload.port),
        sni: Set(payload.sni),
        host: Set(payload.host),
        path: Set(payload.path),
        security: Set(payload.security),
        alpn: Set(payload.alpn),
        fingerprint: Set(payload.fingerprint),
        is_active: Set(payload.is_active.unwrap_or(true)),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;
    Ok(ApiResponse::new(host_to_json(inserted)))
}

pub async fn get_host(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let h = inbound_host::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("host not found"))?;
    Ok(ApiResponse::new(host_to_json(h)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateHostPayload {
    pub remark: Option<String>,
    pub address: Option<String>,
    pub port: Option<i32>,
    pub sni: Option<String>,
    pub host: Option<String>,
    pub path: Option<String>,
    pub security: Option<String>,
    pub alpn: Option<String>,
    pub fingerprint: Option<String>,
    pub is_active: Option<bool>,
}

pub async fn update_host(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateHostPayload>,
) -> ApiResult<Value> {
    let h = inbound_host::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("host not found"))?;

    let mut active: inbound_host::ActiveModel = h.into();

    if let Some(remark) = payload.remark {
        active.remark = Set(remark);
    }
    if let Some(address) = payload.address {
        active.address = Set(address);
    }
    if let Some(port) = payload.port {
        active.port = Set(port);
    }
    if payload.sni.is_some() {
        active.sni = Set(payload.sni);
    }
    if payload.host.is_some() {
        active.host = Set(payload.host);
    }
    if payload.path.is_some() {
        active.path = Set(payload.path);
    }
    if payload.security.is_some() {
        active.security = Set(payload.security);
    }
    if payload.alpn.is_some() {
        active.alpn = Set(payload.alpn);
    }
    if payload.fingerprint.is_some() {
        active.fingerprint = Set(payload.fingerprint);
    }
    if let Some(is_active) = payload.is_active {
        active.is_active = Set(is_active);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;
    Ok(ApiResponse::new(host_to_json(updated)))
}

pub async fn delete_host(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = inbound_host::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("host not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}
