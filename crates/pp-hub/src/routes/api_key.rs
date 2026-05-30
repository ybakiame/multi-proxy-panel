use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::api_key;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

/// List all API keys (excluding the raw key hash).
pub async fn list_keys(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let keys = api_key::Entity::find()
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<Value> = keys
        .into_iter()
        .map(|k| {
            json!({
                "id": k.id,
                "name": k.name,
                "scopes": k.scopes,
                "ip_allowlist": k.ip_allowlist,
                "rate_limit": k.rate_limit,
                "expires_at": k.expires_at,
                "is_active": k.is_active,
                "created_at": k.created_at,
            })
        })
        .collect();

    Ok(Json(json!({ "data": data })))
}

/// Create a new API key. Returns the raw key once (not stored in plain text).
pub async fn create_key(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let raw_key = format!("ck_{}", Uuid::new_v4().simple());
    let key_hash = sha256_truncated(&raw_key);

    let scopes = payload
        .get("scopes")
        .cloned()
        .unwrap_or(json!([]));

    let active = api_key::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        key_hash: Set(key_hash),
        scopes: Set(scopes),
        ip_allowlist: Set(payload.get("ip_allowlist").cloned()),
        rate_limit: Set(payload
            .get("rate_limit")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32)),
        expires_at: Set(None),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active
        .insert(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "data": {
            "id": inserted.id,
            "name": inserted.name,
            "key": raw_key,
            "scopes": inserted.scopes,
            "is_active": inserted.is_active,
        }
    })))
}

/// Update an API key (name, scopes, ip_allowlist, rate_limit, is_active, expires_at).
pub async fn update_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let k = api_key::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: api_key::ActiveModel = k.into();

    if let Some(name) = payload.get("name").and_then(|v| v.as_str()) {
        active.name = Set(name.to_string());
    }
    if payload.get("scopes").is_some() {
        active.scopes = Set(payload.get("scopes").cloned().unwrap_or(json!([])));
    }
    if payload.get("ip_allowlist").is_some() {
        active.ip_allowlist = Set(payload.get("ip_allowlist").cloned());
    }
    if let Some(rate) = payload.get("rate_limit").and_then(|v| v.as_i64()) {
        active.rate_limit = Set(Some(rate as i32));
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
        "scopes": updated.scopes,
        "is_active": updated.is_active,
    } })))
}

/// Delete an API key.
pub async fn delete_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = api_key::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if res.rows_affected == 0 {
        Err(StatusCode::NOT_FOUND)
    } else {
        Ok(StatusCode::NO_CONTENT)
    }
}

fn sha256_truncated(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..16])
}
