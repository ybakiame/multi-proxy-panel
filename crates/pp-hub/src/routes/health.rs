use axum::extract::State;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

pub async fn health(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let db_ok = state.db.ping().await.is_ok();

    Ok(ApiResponse::new(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "database": if db_ok { "connected" } else { "disconnected" },
        "version": env!("CARGO_PKG_VERSION"),
    })))
}

pub async fn ready(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let db_ok = state.db.ping().await.is_ok();
    if !db_ok {
        return Err(ApiError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "service_unavailable",
            "database is not ready",
        ));
    }

    Ok(ApiResponse::new(json!({
        "status": "ready",
        "database": "connected",
        "version": env!("CARGO_PKG_VERSION"),
    })))
}
