use axum::extract::{Query, State};
use pp_db::entities::system_log;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;

use crate::response::{ApiError, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

pub async fn query_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let level = params.get("level");
    let source = params.get("source");

    let mut query = system_log::Entity::find();

    if let Some(l) = level {
        query = query.filter(system_log::Column::Level.eq(l));
    }
    if let Some(s) = source {
        query = query.filter(system_log::Column::Source.contains(s));
    }

    let (records, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = query
                .order_by_desc(system_log::Column::CreatedAt)
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let limit = params
                .get("limit")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(100);
            let items = query
                .order_by_desc(system_log::Column::CreatedAt)
                .limit(limit)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let data: Vec<Value> = records
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "level": r.level,
                "source": r.source,
                "message": r.message,
                "metadata": r.metadata,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(PaginatedResponse::new(data, total))
}
