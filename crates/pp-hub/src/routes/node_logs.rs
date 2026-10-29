use axum::extract::{Path, Query, State};
use pp_db::entities::{agent_log, node};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

pub async fn query_node_logs(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let _ = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node not found"))?;

    let level = params.get("level");
    let mut query = agent_log::Entity::find().filter(agent_log::Column::NodeId.eq(id));

    if let Some(l) = level {
        query = query.filter(agent_log::Column::Level.eq(l));
    }

    let (records, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = query
                .order_by_desc(agent_log::Column::CreatedAt)
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
                .order_by_desc(agent_log::Column::CreatedAt)
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
                "node_id": r.node_id,
                "level": r.level,
                "target": r.target,
                "message": r.message,
                "fields": r.fields,
                "created_at": r.created_at,
            })
        })
        .collect();

    Ok(PaginatedResponse::new(data, total))
}
