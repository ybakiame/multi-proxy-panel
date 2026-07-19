use axum::extract::{Query, State};
use pp_db::entities::node_user_usage_record;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::routes::common::parse_rfc3339;
use crate::state::AppState;

/// Build a query over node_user_usage_records from the shared filter params
/// (`node_id`, `client_id`, `start`, `end`).
fn filtered_query(params: &HashMap<String, String>) -> Select<node_user_usage_record::Entity> {
    let mut query = node_user_usage_record::Entity::find();

    if let Some(node_id) = params.get("node_id").and_then(|s| Uuid::parse_str(s).ok()) {
        query = query.filter(node_user_usage_record::Column::NodeId.eq(node_id));
    }
    if let Some(client_id) = params
        .get("client_id")
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        query = query.filter(node_user_usage_record::Column::ClientId.eq(client_id));
    }
    if let Some(start) = params.get("start").and_then(|s| parse_rfc3339(s)) {
        query = query.filter(node_user_usage_record::Column::HourBucket.gte(start));
    }
    if let Some(end) = params.get("end").and_then(|s| parse_rfc3339(s)) {
        query = query.filter(node_user_usage_record::Column::HourBucket.lte(end));
    }

    query
}

/// GET /api/v1/usage — list per-user hourly usage records, newest first.
pub async fn query_usage(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Value>> {
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(500);

    let records = filtered_query(&params)
        .order_by_desc(node_user_usage_record::Column::HourBucket)
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
                "client_id": r.client_id,
                "hour_bucket": r.hour_bucket,
                "upload_bytes": r.upload_bytes,
                "download_bytes": r.download_bytes,
                "rate": r.rate,
            })
        })
        .collect();

    Ok(ApiResponse::new(data))
}

/// GET /api/v1/usage/summary — aggregate usage grouped by client or node.
pub async fn usage_summary(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Value>> {
    let group_by = params
        .get("group_by")
        .map(String::as_str)
        .unwrap_or("client");
    if !matches!(group_by, "client" | "node") {
        return Err(ApiError::bad_request(
            "invalid_group_by",
            "group_by must be 'client' or 'node'",
        ));
    }
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(20);

    let records = filtered_query(&params)
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let mut totals: HashMap<Uuid, (i64, i64)> = HashMap::new();
    for r in records {
        let key = if group_by == "node" {
            r.node_id
        } else {
            r.client_id
        };
        let entry = totals.entry(key).or_default();
        entry.0 += r.upload_bytes;
        entry.1 += r.download_bytes;
    }

    let mut rows: Vec<(Uuid, i64, i64)> = totals
        .into_iter()
        .map(|(id, (upload, download))| (id, upload, download))
        .collect();
    rows.sort_by_key(|(_, upload, download)| std::cmp::Reverse(upload + download));
    rows.truncate(limit);

    let data: Vec<Value> = rows
        .into_iter()
        .map(|(id, upload, download)| {
            json!({
                "id": id,
                "upload_bytes": upload,
                "download_bytes": download,
                "total_bytes": upload + download,
            })
        })
        .collect();

    Ok(ApiResponse::new(data))
}
