use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use pp_db::entities::traffic_record;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

pub async fn query_traffic(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, StatusCode> {
    let node_id = params.get("node_id").and_then(|s| Uuid::parse_str(s).ok());
    let client_id = params
        .get("client_id")
        .and_then(|s| Uuid::parse_str(s).ok());

    let mut query = traffic_record::Entity::find();

    if let Some(id) = node_id {
        query = query.filter(traffic_record::Column::NodeId.eq(id));
    }
    if let Some(id) = client_id {
        query = query.filter(traffic_record::Column::ClientId.eq(id));
    }

    let records = query
        .order_by_desc(traffic_record::Column::HourBucket)
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    Ok(Json(json!({ "data": data })))
}
