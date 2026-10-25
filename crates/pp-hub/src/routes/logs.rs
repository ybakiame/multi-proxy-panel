//! Central log queries; system is the backward-compatible default.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use pp_db::entities::{agent_log, node, system_log};
use sea_orm::{
    ColumnTrait, Condition, EntityTrait, JoinType, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, RelationTrait,
};
use serde_json::{Value, json};

use crate::response::{ApiError, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

pub async fn query_logs(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let kind = params.get("kind").map_or("system", String::as_str);
    if !["system", "agent"].contains(&kind) {
        return Err(ApiError::bad_request(
            "invalid_log_kind",
            "kind must be system or agent",
        ));
    }
    let level = params
        .get("level")
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty() && s != "all");
    if level
        .as_ref()
        .is_some_and(|s| !["info", "warn", "error", "debug", "trace"].contains(&s.as_str()))
    {
        return Err(ApiError::bad_request(
            "invalid_log_level",
            "unsupported log level",
        ));
    }
    let source = params
        .get("source")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty());
    if source.is_some_and(|s| s.len() > 256) {
        return Err(ApiError::bad_request(
            "invalid_log_source",
            "source is too long",
        ));
    }
    let pagination = crate::routes::common::parse_pagination(&params);
    let (offset, limit) = match pagination {
        Some((page, per_page)) => {
            let size = per_page.min(200);
            ((page - 1).saturating_mul(size), size)
        }
        None => (
            0,
            params
                .get("limit")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(100)
                .clamp(1, 200),
        ),
    };
    let clean = pp_common::logging::sanitize_log_text;
    let (data, total) = if kind == "agent" {
        let mut query =
            agent_log::Entity::find().join(JoinType::LeftJoin, agent_log::Relation::Node.def());
        if let Some(level) = &level {
            query = query.filter(agent_log::Column::Level.eq(level));
        }
        if let Some(source) = source {
            query = query.filter(
                Condition::any()
                    .add(agent_log::Column::Target.contains(source))
                    .add(node::Column::Name.contains(source)),
            );
        }
        let total = if pagination.is_some() {
            query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)?
        } else {
            0
        };
        let records = query
            .order_by_desc(agent_log::Column::CreatedAt)
            .order_by_desc(agent_log::Column::Id)
            .offset(offset)
            .limit(limit)
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        let ids: Vec<_> = records.iter().map(|r| r.node_id).collect();
        let names: HashMap<uuid::Uuid, String> = node::Entity::find()
            .select_only()
            .columns([node::Column::Id, node::Column::Name])
            .filter(node::Column::Id.is_in(ids))
            .into_tuple()
            .all(&state.db)
            .await
            .map_err(ApiError::from)?
            .into_iter()
            .collect();
        let data: Vec<_> = records.into_iter().map(|r| json!({"id": r.id, "level": r.level, "source": r.target, "node_id": r.node_id,
            "node_name": names.get(&r.node_id), "message": clean(&r.message), "metadata": r.fields.map(pp_common::logging::sanitize_log_metadata), "created_at": r.created_at})).collect();
        let total = if pagination.is_some() {
            total
        } else {
            data.len() as u64
        };
        (data, total)
    } else {
        let mut query = system_log::Entity::find();
        if let Some(level) = &level {
            query = query.filter(system_log::Column::Level.eq(level));
        }
        if let Some(source) = source {
            query = query.filter(system_log::Column::Source.contains(source));
        }
        let total = if pagination.is_some() {
            query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)?
        } else {
            0
        };
        let records = query
            .order_by_desc(system_log::Column::CreatedAt)
            .order_by_desc(system_log::Column::Id)
            .offset(offset)
            .limit(limit)
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        let data: Vec<_> = records.into_iter().map(|r| json!({"id": r.id, "level": r.level, "source": r.source, "message": clean(&r.message), "metadata": r.metadata.map(pp_common::logging::sanitize_log_metadata), "created_at": r.created_at})).collect();
        let total = if pagination.is_some() {
            total
        } else {
            data.len() as u64
        };
        (data, total)
    };
    Ok(PaginatedResponse::new(data, total))
}

#[cfg(test)]
#[path = "logs_tests.rs"]
mod tests;
