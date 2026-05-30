use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::protocol_config;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QuerySelect, Set};
use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

pub async fn list_configs(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let page = params.get("page").and_then(|s| s.parse::<u64>().ok()).unwrap_or(1).max(1);
    let per_page = params.get("per_page").and_then(|s| s.parse::<u64>().ok()).unwrap_or(20).max(1).min(100);

    let total = protocol_config::Entity::find()
        .count(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? as u64;

    let configs = protocol_config::Entity::find()
        .offset(((page - 1) * per_page) as u64)
        .limit(per_page)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<Value> = configs.into_iter().map(|c| json!({
        "id": c.id,
        "name": c.name,
        "protocol_type": c.protocol_type,
        "core_type": c.core_type,
        "listen_port": c.listen_port,
        "listen_address": c.listen_address,
        "settings": c.settings,
        "tls_settings": c.tls_settings,
    })).collect();
    Ok(Json(json!({ "data": data, "total": total })))
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, StatusCode> {
    let cfg = protocol_config::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(json!({ "data": {
        "id": cfg.id,
        "name": cfg.name,
        "protocol_type": cfg.protocol_type,
        "core_type": cfg.core_type,
        "listen_port": cfg.listen_port,
        "listen_address": cfg.listen_address,
        "settings": cfg.settings,
        "tls_settings": cfg.tls_settings,
    } })))
}

fn validate_protocol(payload: &Value) -> Result<(String, String), StatusCode> {
    let protocol_type = payload
        .get("protocol_type")
        .and_then(|v| v.as_str())
        .unwrap_or("vless_reality");
    let core_type = payload
        .get("core_type")
        .and_then(|v| v.as_str())
        .unwrap_or("xray");

    let allowed = match protocol_type {
        "vless_reality" | "vless_vision" => &["xray", "sing-box"][..],
        "vless_xhttp" => &["xray"][..],
        "hysteria2" => &["sing-box"][..],
        "anytls" => &["sing-box"][..],
        "tuic" | "tuic_v5" => &["sing-box"][..],
        _ => return Err(StatusCode::BAD_REQUEST),
    };

    if !allowed.contains(&core_type) {
        return Err(StatusCode::BAD_REQUEST);
    }

    Ok((protocol_type.to_string(), core_type.to_string()))
}

pub async fn create_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let (protocol_type, core_type) = validate_protocol(&payload)?;

    let active = protocol_config::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        protocol_type: Set(protocol_type),
        core_type: Set(core_type),
        listen_port: Set(
            payload
                .get("listen_port")
                .and_then(|v| v.as_u64())
                .unwrap_or(443) as i32,
        ),
        listen_address: Set(
            payload
                .get("listen_address")
                .and_then(|v| v.as_str())
                .unwrap_or("0.0.0.0")
                .to_string(),
        ),
        settings: Set(payload.get("settings").cloned().unwrap_or(json!({}))),
        tls_settings: Set(payload.get("tls_settings").cloned()),
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
        "protocol_type": inserted.protocol_type,
        "core_type": inserted.core_type,
        "listen_port": inserted.listen_port,
        "listen_address": inserted.listen_address,
    } })))
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let cfg = protocol_config::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: protocol_config::ActiveModel = cfg.into();

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        active.name = Set(name.to_string());
    }
    if payload.get("protocol_type").is_some() || payload.get("core_type").is_some() {
        let (_, _) = validate_protocol(&payload)?;
        if let Some(pt) = payload.get("protocol_type").and_then(|v| v.as_str()) {
            active.protocol_type = Set(pt.to_string());
        }
        if let Some(ct) = payload.get("core_type").and_then(|v| v.as_str()) {
            active.core_type = Set(ct.to_string());
        }
    }
    if let Some(port) = payload.get("listen_port").and_then(|v| v.as_u64()) {
        active.listen_port = Set(port as i32);
    }
    if let Some(addr) = payload.get("listen_address").and_then(|v| v.as_str()) {
        active.listen_address = Set(addr.to_string());
    }
    if let Some(settings) = payload.get("settings") {
        active.settings = Set(settings.clone());
    }
    if payload.get("tls_settings").is_some() {
        active.tls_settings = Set(payload.get("tls_settings").cloned());
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active
        .update(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": updated.id,
        "name": updated.name,
        "protocol_type": updated.protocol_type,
        "core_type": updated.core_type,
        "listen_port": updated.listen_port,
        "listen_address": updated.listen_address,
    } })))
}

pub async fn generate_reality_keys() -> Result<Json<Value>, StatusCode> {
    let (private_key, public_key) = pp_common::generate_x25519_keypair();
    let short_id = pp_common::generate_short_id();
    Ok(Json(json!({
        "data": {
            "private_key": private_key,
            "public_key": public_key,
            "short_id": short_id,
        }
    })))
}

pub async fn delete_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = protocol_config::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}
