use axum::extract::{Query, State};
use pp_db::entities::traffic_record;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::routes::common::parse_rfc3339;
use crate::state::AppState;

const DEFAULT_LIMIT: u64 = 500;
const MAX_LIMIT: u64 = 5000;

pub async fn query_traffic(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Value>> {
    let node_id = params.get("node_id").and_then(|s| Uuid::parse_str(s).ok());
    let client_id = params
        .get("client_id")
        .and_then(|s| Uuid::parse_str(s).ok());
    let limit = params
        .get("limit")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT);

    let mut query = traffic_record::Entity::find();

    if let Some(id) = node_id {
        query = query.filter(traffic_record::Column::NodeId.eq(id));
    }
    if let Some(id) = client_id {
        query = query.filter(traffic_record::Column::ClientId.eq(id));
    }
    if let Some(start) = params.get("start").and_then(|s| parse_rfc3339(s)) {
        query = query.filter(traffic_record::Column::HourBucket.gte(start));
    }
    if let Some(end) = params.get("end").and_then(|s| parse_rfc3339(s)) {
        query = query.filter(traffic_record::Column::HourBucket.lte(end));
    }

    let records = query
        .order_by_desc(traffic_record::Column::HourBucket)
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
                "protocol_config_id": r.protocol_config_id,
                "client_id": r.client_id,
                "hour_bucket": r.hour_bucket,
                "upload_bytes": r.upload_bytes,
                "download_bytes": r.download_bytes,
            })
        })
        .collect();

    Ok(ApiResponse::new(data))
}
