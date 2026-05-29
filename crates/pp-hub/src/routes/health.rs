use axum::{extract::State, response::Json};
use serde_json::json;
use std::sync::Arc;

use crate::state::AppState;

pub async fn health(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    // Basic health check including DB connectivity
    let db_ok = state.db.ping().await.is_ok();

    Json(json!({
        "status": if db_ok { "healthy" } else { "degraded" },
        "database": if db_ok { "connected" } else { "disconnected" },
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
