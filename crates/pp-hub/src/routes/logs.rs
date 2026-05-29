use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::system_log;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use sea_orm::EntityTrait as _;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::state::AppState;

pub async fn query_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let level = params.get("level");
    let source = params.get("source");
    let limit = params.get("limit").and_then(|s| s.parse::<u64>().ok()).unwrap_or(100);

    let mut query = system_log::Entity::find();

    if let Some(l) = level {
        query = query.filter(system_log::Column::Level.eq(l));
    }
    if let Some(s) = source {
        query = query.filter(system_log::Column::Source.contains(s));
    }

    let records = query
        .order_by_desc(system_log::Column::CreatedAt)
        .limit(limit)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let data: Vec<Value> = records.into_iter().map(|r| json!({
        "id": r.id,
        "level": r.level,
        "source": r.source,
        "message": r.message,
        "metadata": r.metadata,
        "created_at": r.created_at,
    })).collect();

    Ok(Json(json!({ "data": data })))
}
