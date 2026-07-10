use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::api_key;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QuerySelect, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::middleware::api_key::ApiKeyAuth;
use crate::middleware::api_key::scopes;
use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn json_array(value: Option<&Value>) -> Value {
    value
        .and_then(|v| v.as_array())
        .map(|a| json!(a))
        .unwrap_or_else(|| json!([]))
}

fn key_to_json(k: api_key::Model) -> Value {
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
}

/// List all API keys (excluding the raw key hash).
pub async fn list_keys(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (keys, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = api_key::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = api_key::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = api_key::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = keys.into_iter().map(key_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateKeyPayload {
    pub name: String,
    pub scopes: Option<Value>,
    pub ip_allowlist: Option<Value>,
    pub rate_limit: Option<i64>,
}

/// Create a new API key. Returns the raw key once (not stored in plain text).
#[axum::debug_handler]
pub async fn create_key(
    _auth: ApiKeyAuth,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateKeyPayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "API key name is required",
        ));
    }

    let raw_key = format!("ck_{}", Uuid::new_v4().simple());
    let key_hash = pp_common::hash_secret_async(raw_key.clone())
        .await
        .map_err(|e| ApiError::internal(format!("failed to hash key: {e}")))?;

    let scopes = json_array(payload.scopes.as_ref());
    validate_scopes(&scopes).map_err(|e| ApiError::bad_request("invalid_scopes", e))?;

    let active = api_key::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        key_hash: Set(key_hash),
        scopes: Set(scopes),
        ip_allowlist: Set(payload.ip_allowlist),
        rate_limit: Set(payload.rate_limit.map(|v| v as i32)),
        expires_at: Set(None),
        is_active: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    state.api_key_cache.invalidate();

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "name": inserted.name,
        "key": raw_key,
        "scopes": inserted.scopes,
        "is_active": inserted.is_active,
    })))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateKeyPayload {
    pub name: Option<String>,
    pub scopes: Option<Value>,
    pub ip_allowlist: Option<Value>,
    pub rate_limit: Option<i64>,
    pub is_active: Option<bool>,
}

/// Update an API key (name, scopes, ip_allowlist, rate_limit, is_active, expires_at).
pub async fn update_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateKeyPayload>,
) -> ApiResult<Value> {
    let k = api_key::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("API key not found"))?;

    let mut active: api_key::ActiveModel = k.into();

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "API key name cannot be empty",
            ));
        }
        active.name = Set(name);
    }
    if payload.scopes.is_some() {
        let scopes = json_array(payload.scopes.as_ref());
        validate_scopes(&scopes).map_err(|e| ApiError::bad_request("invalid_scopes", e))?;
        active.scopes = Set(scopes);
    }
    if payload.ip_allowlist.is_some() {
        active.ip_allowlist = Set(payload.ip_allowlist);
    }
    if let Some(rate) = payload.rate_limit {
        active.rate_limit = Set(Some(rate as i32));
    }
    if let Some(active_flag) = payload.is_active {
        active.is_active = Set(active_flag);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    state.api_key_cache.invalidate();

    Ok(ApiResponse::new(key_to_json(updated)))
}

/// Delete an API key.
pub async fn delete_key(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = api_key::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("API key not found"))
    } else {
        state.api_key_cache.invalidate();
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

fn validate_scopes(scopes: &Value) -> Result<(), &'static str> {
    let arr = scopes.as_array().ok_or("scopes must be an array")?;
    for v in arr {
        let s = v.as_str().ok_or("scope must be a string")?;
        if s == "*" || scopes::ALL_SCOPES.contains(&s) {
            continue;
        }
        return Err("unknown scope");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_scopes_accepts_known() {
        assert!(validate_scopes(&json!(["nodes:read", "nodes:write"])).is_ok());
        assert!(validate_scopes(&json!(["*"])).is_ok());
    }

    #[test]
    fn validate_scopes_rejects_unknown() {
        assert!(validate_scopes(&json!(["invalid:scope"])).is_err());
    }
}
