use axum::{Json, extract::State};
use pp_db::entities::user;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::sync::Arc;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct LoginPayload {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub username: String,
    pub role: String,
}

/// POST /api/v1/login
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<LoginPayload>,
) -> ApiResult<Value> {
    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_input",
            "username and password are required",
        ));
    }

    let user_record = user::Entity::find()
        .filter(user::Column::Username.eq(&payload.username))
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| {
            ApiError::new(
                axum::http::StatusCode::UNAUTHORIZED,
                "invalid_credentials",
                "invalid username or password",
            )
        })?;

    let valid =
        pp_common::verify_secret(&payload.password, &user_record.password_hash).unwrap_or(false);
    if !valid {
        return Err(ApiError::new(
            axum::http::StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "invalid username or password",
        ));
    }

    let token = crate::middleware::auth::create_jwt(
        user_record.id,
        &user_record.username,
        &user_record.role,
        &state.config.jwt_secret,
        24,
    )
    .map_err(|e| {
        tracing::error!("JWT creation failed: {}", e);
        ApiError::internal("failed to create token")
    })?;

    Ok(ApiResponse::new(json!({
        "token": token,
        "user_id": user_record.id,
        "username": user_record.username,
        "role": user_record.role,
    })))
}

/// GET /api/v1/me — Get current authenticated user info.
pub async fn me(auth: crate::middleware::auth::AuthUser) -> ApiResult<Value> {
    Ok(ApiResponse::new(json!({
        "user_id": auth.user_id,
        "username": auth.username,
        "role": auth.role,
    })))
}

#[derive(serde::Deserialize)]
pub struct CreateUserPayload {
    pub username: String,
    pub password: String,
    pub role: Option<String>,
}

/// POST /api/v1/users — Create a new admin user (requires existing admin).
pub async fn create_user(
    State(state): State<Arc<AppState>>,
    auth: crate::middleware::auth::AuthUser,
    Json(payload): Json<CreateUserPayload>,
) -> ApiResult<Value> {
    if auth.role != "admin" {
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "forbidden",
            "only admins can create users",
        ));
    }

    if payload.username.trim().is_empty() || payload.password.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_input",
            "username and password are required",
        ));
    }

    if payload.password.len() < 8 {
        return Err(ApiError::bad_request(
            "weak_password",
            "password must be at least 8 characters",
        ));
    }

    let password_hash = pp_common::hash_secret(&payload.password)
        .map_err(|e| ApiError::internal(format!("failed to hash password: {e}")))?;

    let active = user::ActiveModel {
        id: Set(uuid::Uuid::new_v4()),
        username: Set(payload.username.trim().to_string()),
        password_hash: Set(password_hash),
        role: Set(payload.role.unwrap_or_else(|| "admin".to_string())),
        status: Set("active".to_string()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "username": inserted.username,
        "role": inserted.role,
    })))
}
