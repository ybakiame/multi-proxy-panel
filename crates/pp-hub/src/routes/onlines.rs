use axum::{
    extract::{Path, Query, State},
};
use pp_db::entities::client_online_session;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

/// List current online sessions (active within last 5 minutes).
pub async fn list_onlines(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> ApiResult<Vec<Value>> {
    let five_min_ago = chrono::Utc::now() - chrono::Duration::minutes(5);

    let mut query = client_online_session::Entity::find()
        .filter(client_online_session::Column::LastActiveAt.gte(five_min_ago));

    if let Some(node_id) = params.get("node_id").and_then(|s| Uuid::parse_str(s).ok()) {
        query = query.filter(client_online_session::Column::NodeId.eq(node_id));
    }
    if let Some(client_id) = params
        .get("client_id")
        .and_then(|s| Uuid::parse_str(s).ok())
    {
        query = query.filter(client_online_session::Column::ClientId.eq(client_id));
    }

    let sessions = query
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let data: Vec<Value> = sessions
        .into_iter()
        .map(|s| {
            json!({
                "id": s.id,
                "client_id": s.client_id,
                "node_id": s.node_id,
                "ip_address": s.ip_address,
                "inbound_tag": s.inbound_tag,
                "connected_at": s.connected_at,
                "last_active_at": s.last_active_at,
            })
        })
        .collect();

    Ok(ApiResponse::new(data))
}

/// Get unique IP addresses for a client.
pub async fn get_client_ips(
    State(state): State<Arc<AppState>>,
    Path(client_id): Path<Uuid>,
) -> ApiResult<Value> {
    let sessions = client_online_session::Entity::find()
        .filter(client_online_session::Column::ClientId.eq(client_id))
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let ips: std::collections::HashSet<String> =
        sessions.into_iter().map(|s| s.ip_address).collect();
    let unique_ips: Vec<String> = ips.into_iter().collect();

    Ok(ApiResponse::new(
        json!({ "client_id": client_id, "ips": unique_ips }),
    ))
}

/// Get total count of currently online users.
pub async fn get_online_count(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Value> {
    let five_min_ago = chrono::Utc::now() - chrono::Duration::minutes(5);

    let sessions = client_online_session::Entity::find()
        .filter(client_online_session::Column::LastActiveAt.gte(five_min_ago))
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let unique_clients: std::collections::HashSet<Uuid> =
        sessions.into_iter().map(|s| s.client_id).collect();
    let count = unique_clients.len() as i64;

    Ok(ApiResponse::new(json!({ "count": count })))
}
