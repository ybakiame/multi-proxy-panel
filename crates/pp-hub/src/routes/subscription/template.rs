use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::subscription_template;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Subscription Templates
// ---------------------------------------------------------------------------

fn template_to_json(t: subscription_template::Model) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "format": t.format,
        "base_config": t.base_config,
        "filter_rules": t.filter_rules,
        "custom_headers": t.custom_headers,
        "is_builtin": t.is_builtin,
        "is_enabled": t.is_enabled,
        "created_at": t.created_at,
        "updated_at": t.updated_at,
    })
}

pub async fn list_templates(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let items = subscription_template::Entity::find()
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let total = items.len() as u64;
    let templates: Vec<Value> =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            items
                .into_iter()
                .skip(((page - 1) * per_page) as usize)
                .take(per_page as usize)
                .map(template_to_json)
                .collect()
        } else {
            items.into_iter().map(template_to_json).collect()
        };

    Ok(PaginatedResponse::new(templates, total))
}

#[derive(serde::Deserialize)]
pub struct CreateTemplatePayload {
    pub name: String,
    pub format: Option<String>,
    pub base_config: Option<String>,
    pub filter_rules: Option<Value>,
    pub custom_headers: Option<Value>,
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTemplatePayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "template name is required",
        ));
    }

    let format = payload.format.unwrap_or_else(|| "base64".to_string());
    let active = subscription_template::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        name: Set(payload.name),
        format: Set(format.clone()),
        base_config: Set(payload.base_config),
        filter_rules: Set(payload.filter_rules),
        custom_headers: Set(payload.custom_headers),
        is_builtin: Set(false),
        is_enabled: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    // Enforce only one enabled template per format.
    enforce_unique_enabled_template(&state.db, inserted.id.clone(), &format, true).await?;

    Ok(ApiResponse::new(template_to_json(inserted)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateTemplatePayload {
    pub name: Option<String>,
    pub format: Option<String>,
    pub base_config: Option<String>,
    pub filter_rules: Option<Value>,
    pub custom_headers: Option<Value>,
    pub is_enabled: Option<bool>,
}

pub async fn update_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTemplatePayload>,
) -> ApiResult<Value> {
    let template = subscription_template::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription template not found"))?;

    // Builtin templates may only have their enabled flag changed.
    if template.is_builtin
        && (payload.name.is_some()
            || payload.format.is_some()
            || payload.base_config.is_some()
            || payload.filter_rules.is_some()
            || payload.custom_headers.is_some())
    {
        return Err(ApiError::bad_request(
            "builtin_readonly",
            "builtin templates can only be enabled or disabled",
        ));
    }

    let mut active: subscription_template::ActiveModel = template.clone().into();
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "template name is required",
            ));
        }
        active.name = Set(name);
    }
    let mut new_format = template.format.clone();
    if let Some(format) = payload.format {
        active.format = Set(format.clone());
        new_format = format;
    }
    if payload.base_config.is_some() {
        active.base_config = Set(payload.base_config);
    }
    if payload.filter_rules.is_some() {
        active.filter_rules = Set(payload.filter_rules);
    }
    if payload.custom_headers.is_some() {
        active.custom_headers = Set(payload.custom_headers);
    }
    let mut new_enabled = template.is_enabled;
    if let Some(enabled) = payload.is_enabled {
        active.is_enabled = Set(enabled);
        new_enabled = enabled;
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    // Enforce only one enabled template per format.
    enforce_unique_enabled_template(&state.db, updated.id.clone(), &new_format, new_enabled)
        .await?;

    Ok(ApiResponse::new(template_to_json(updated)))
}

pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let template = subscription_template::Entity::find_by_id(id.clone())
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription template not found"))?;

    if template.is_builtin {
        return Err(ApiError::bad_request(
            "builtin_protected",
            "builtin templates cannot be deleted",
        ));
    }

    let res = subscription_template::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;
    if res.rows_affected == 0 {
        Err(ApiError::not_found("subscription template not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

/// Ensure at most one template per format is enabled.
/// When `enabled` is true, disable all other enabled templates of the same format.
async fn enforce_unique_enabled_template(
    db: &sea_orm::DatabaseConnection,
    template_id: String,
    format: &str,
    enabled: bool,
) -> Result<(), ApiError> {
    if !enabled {
        return Ok(());
    }

    let others = subscription_template::Entity::find()
        .filter(subscription_template::Column::Id.ne(template_id))
        .filter(subscription_template::Column::Format.eq(format))
        .filter(subscription_template::Column::IsEnabled.eq(true))
        .all(db)
        .await
        .map_err(ApiError::from)?;

    for other in others {
        let mut active: subscription_template::ActiveModel = other.into();
        active.is_enabled = Set(false);
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(db).await.map_err(ApiError::from)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Subscriptions (admin CRUD)
// ---------------------------------------------------------------------------

fn subscription_to_json(s: pp_db::entities::subscription::Model) -> Value {
    json!({
        "id": s.id,
        "client_id": s.client_id,
        "token": s.token,
        "url_path": s.url_path,
        "is_active": s.is_active,
        "expire_at": s.expire_at.map(|d| d.to_rfc3339()),
        "last_accessed_at": s.last_accessed_at.map(|d| d.to_rfc3339()),
        "created_at": s.created_at,
    })
}

pub async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (subs, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = pp_db::entities::subscription::Entity::find()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = pp_db::entities::subscription::Entity::find()
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = pp_db::entities::subscription::Entity::find()
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = subs.into_iter().map(subscription_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateSubscriptionPayload {
    pub client_id: Uuid,
}

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSubscriptionPayload>,
) -> ApiResult<Value> {
    let token = pp_common::generate_secure_token();
    let url_path = format!("/sub/{}", token);

    let active = pp_db::entities::subscription::ActiveModel {
        id: Set(Uuid::new_v4()),
        client_id: Set(payload.client_id),
        token: Set(token.clone()),
        url_path: Set(url_path.clone()),
        expire_at: Set(None),
        is_active: Set(true),
        last_accessed_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "token": token,
        "url_path": url_path,
    })))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateSubscriptionPayload {
    pub is_active: Option<bool>,
    pub expire_at: Option<String>,
}

pub async fn update_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubscriptionPayload>,
) -> ApiResult<Value> {
    let sub = pp_db::entities::subscription::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription not found"))?;

    let mut active: pp_db::entities::subscription::ActiveModel = sub.into();

    if let Some(is_active) = payload.is_active {
        active.is_active = Set(is_active);
    }
    if payload.expire_at.is_some() {
        let expire_at = payload
            .expire_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok());
        active.expire_at = Set(expire_at);
    }

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(subscription_to_json(updated)))
}

pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = pp_db::entities::subscription::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;
    if res.rows_affected == 0 {
        Err(ApiError::not_found("subscription not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}
