use axum::extract::{Path, Query, State};
use pp_db::entities::host_metric;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

pub async fn query_metrics(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Value>> {
    let node_id = params.get("node_id").and_then(|s| Uuid::parse_str(s).ok());
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(100);

    let mut query = host_metric::Entity::find();

    if let Some(id) = node_id {
        query = query.filter(host_metric::Column::NodeId.eq(id));
    }

    let records = query
        .order_by_desc(host_metric::Column::Timestamp)
        .limit(limit)
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let data: Vec<Value> = records
        .into_iter()
        .map(|r| {
            json!({
                "id": r.id,
                "node_id": r.node_id,
                "timestamp": r.timestamp,
                "cpu_percent": r.cpu_percent,
                "mem_used": r.mem_used,
                "mem_total": r.mem_total,
                "disk_used": r.disk_used,
                "disk_total": r.disk_total,
                "net_rx": r.net_rx,
                "net_tx": r.net_tx,
                "load_avg1": r.load_avg1,
                "load_avg5": r.load_avg5,
                "load_avg15": r.load_avg15,
            })
        })
        .collect();

    Ok(ApiResponse::new(data))
}

pub async fn latest_metrics(
    State(state): State<Arc<AppState>>,
    Path(node_id): Path<Uuid>,
) -> ApiResult<Value> {
    let record = host_metric::Entity::find()
        .filter(host_metric::Column::NodeId.eq(node_id))
        .order_by_desc(host_metric::Column::Timestamp)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("metrics not found for node"))?;

    Ok(ApiResponse::new(json!({
        "node_id": record.node_id,
        "timestamp": record.timestamp,
        "cpu_percent": record.cpu_percent,
        "mem_used": record.mem_used,
        "mem_total": record.mem_total,
        "disk_used": record.disk_used,
        "disk_total": record.disk_total,
        "load_avg": [record.load_avg1, record.load_avg5, record.load_avg15],
    })))
}
