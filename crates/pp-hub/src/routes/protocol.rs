use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::protocol_config;
use sea_orm::{ActiveModelTrait, EntityTrait, PaginatorTrait, QuerySelect, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

fn config_to_json(c: protocol_config::Model) -> Value {
    json!({
        "id": c.id,
        "name": c.name,
        "protocol_type": c.protocol_type,
        "core_type": c.core_type,
        "core_version": c.core_version,
        "listen_port": c.listen_port,
        "listen_address": c.listen_address,
        "settings": c.settings,
        "tls_settings": c.tls_settings,
    })
}

pub async fn list_configs(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (configs, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = protocol_config::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = protocol_config::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = protocol_config::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = configs.into_iter().map(config_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

pub async fn get_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Value> {
    let cfg = protocol_config::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("protocol config not found"))?;

    Ok(ApiResponse::new(config_to_json(cfg)))
}

fn validate_protocol(
    payload: &Value,
) -> Result<(pp_common::ProtocolType, pp_common::CoreType), ApiError> {
    let protocol_str = payload
        .get("protocol_type")
        .and_then(|v| v.as_str())
        .unwrap_or("vless_reality");
    let core_str = payload
        .get("core_type")
        .and_then(|v| v.as_str())
        .unwrap_or("xray");

    let protocol = protocol_str
        .parse::<pp_common::ProtocolType>()
        .map_err(|_| ApiError::bad_request("invalid_protocol_type", "unknown protocol type"))?;
    let core = core_str
        .parse::<pp_common::CoreType>()
        .map_err(|_| ApiError::bad_request("invalid_core_type", "unknown core type"))?;

    let allowed = pp_common::CoreType::valid_for(protocol);
    if !allowed.contains(&core) {
        return Err(ApiError::bad_request(
            "incompatible_core_type",
            format!("{} does not support core {}", protocol, core),
        ));
    }

    Ok((protocol, core))
}

#[derive(serde::Deserialize)]
pub struct CreateConfigPayload {
    pub name: String,
    pub protocol_type: String,
    pub core_type: String,
    pub core_version: Option<String>,
    pub listen_port: Option<u64>,
    pub listen_address: Option<String>,
    pub settings: Option<Value>,
    pub tls_settings: Option<Value>,
}

pub async fn create_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateConfigPayload>,
) -> ApiResult<Value> {
    let (protocol_type, core_type) = validate_protocol(&json!({
        "protocol_type": payload.protocol_type,
        "core_type": payload.core_type,
    }))?;

    let active = protocol_config::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        protocol_type: Set(protocol_type.to_string()),
        core_type: Set(core_type.to_string()),
        core_version: Set(payload.core_version),
        listen_port: Set(payload.listen_port.unwrap_or(443) as i32),
        listen_address: Set(payload
            .listen_address
            .unwrap_or_else(|| "0.0.0.0".to_string())),
        settings: Set(payload.settings.unwrap_or_else(|| json!({}))),
        tls_settings: Set(payload.tls_settings),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(config_to_json(inserted)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateConfigPayload {
    pub name: Option<String>,
    pub protocol_type: Option<String>,
    pub core_type: Option<String>,
    pub core_version: Option<String>,
    pub listen_port: Option<u64>,
    pub listen_address: Option<String>,
    pub settings: Option<Value>,
    pub tls_settings: Option<Value>,
}

pub async fn update_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateConfigPayload>,
) -> ApiResult<Value> {
    let cfg = protocol_config::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("protocol config not found"))?;

    let mut active: protocol_config::ActiveModel = cfg.into();

    if let Some(name) = payload.name {
        active.name = Set(name);
    }
    if payload.protocol_type.is_some() || payload.core_type.is_some() {
        let existing_protocol = active.protocol_type.clone().unwrap();
        let existing_core = active.core_type.clone().unwrap();
        let protocol_type = payload
            .protocol_type
            .as_deref()
            .unwrap_or(&existing_protocol);
        let core_type = payload.core_type.as_deref().unwrap_or(&existing_core);
        let _ = validate_protocol(&json!({
            "protocol_type": protocol_type,
            "core_type": core_type,
        }))?;

        if let Some(pt) = payload.protocol_type {
            active.protocol_type = Set(pt);
        }
        if let Some(ct) = payload.core_type {
            active.core_type = Set(ct);
        }
    }
    if let Some(port) = payload.listen_port {
        active.listen_port = Set(port as i32);
    }
    if let Some(addr) = payload.listen_address {
        active.listen_address = Set(addr);
    }
    if payload.core_version.is_some() {
        active.core_version = Set(payload.core_version);
    }
    if let Some(settings) = payload.settings {
        active.settings = Set(settings);
    }
    active.tls_settings = Set(payload.tls_settings);
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    // Push updated config to all nodes that have an active binding using this config.
    // Failures are logged but do not fail the HTTP request, so the config change is
    // persisted even if an agent is temporarily offline.
    match crate::service::protocol::nodes_using_config(&state.db, updated.id).await {
        Ok(node_ids) => {
            let core_type = updated
                .core_type
                .parse::<pp_common::CoreType>()
                .unwrap_or(pp_common::CoreType::SingBox);
            for node_id in node_ids {
                if let Err(e) = crate::service::protocol::push_node_config(
                    &state,
                    node_id,
                    core_type,
                    true,
                    updated.core_version.clone(),
                )
                .await
                {
                    tracing::warn!(
                        "failed to push config to node {} after updating config {}: {}",
                        node_id,
                        updated.id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!("failed to find nodes using config {}: {}", updated.id, e);
        }
    }

    Ok(ApiResponse::new(config_to_json(updated)))
}

pub async fn generate_reality_keys() -> ApiResult<Value> {
    let (private_key, public_key) = pp_common::generate_x25519_keypair();
    let short_id = pp_common::generate_short_id();
    Ok(ApiResponse::new(json!({
        "private_key": private_key,
        "public_key": public_key,
        "short_id": short_id,
    })))
}

pub async fn delete_config(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = protocol_config::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("protocol config not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}
