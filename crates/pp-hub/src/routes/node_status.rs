use pp_db::entities::{agent_log, node};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::ApiError;
use crate::state::AppState;

fn core_status_from_log(log: &agent_log::Model) -> Option<Value> {
    let fields = log.fields.as_ref()?;
    let core_type = fields.get("core_type")?.as_str()?;
    let version = fields.get("version")?.as_str()?;
    let running = fields.get("running")?.as_bool()?;
    let uptime_sec = fields.get("uptime_sec")?.as_u64()?;
    let last_error = fields.get("last_error")?.as_str().unwrap_or("");
    Some(json!({
        "core_type": core_type,
        "version": version,
        "running": running,
        "uptime_sec": uptime_sec,
        "last_error": last_error,
        "updated_at": log.created_at,
    }))
}

pub(crate) async fn latest_core_statuses(
    state: &Arc<AppState>,
    node_ids: &[Uuid],
) -> Result<std::collections::HashMap<Uuid, Vec<Value>>, ApiError> {
    if node_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let mut by_node: std::collections::HashMap<Uuid, std::collections::HashMap<String, Value>> =
        std::collections::HashMap::new();

    for node_id in node_ids {
        let logs = agent_log::Entity::find()
            .filter(agent_log::Column::NodeId.eq(*node_id))
            .filter(agent_log::Column::Target.like("core-%"))
            .order_by_desc(agent_log::Column::CreatedAt)
            .limit(10)
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;

        for log in logs {
            if let Some(status) = core_status_from_log(&log) {
                let core_type = status
                    .get("core_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                by_node
                    .entry(log.node_id)
                    .or_default()
                    .entry(core_type)
                    .or_insert(status);
            }
        }
    }

    Ok(by_node
        .into_iter()
        .map(|(node_id, statuses)| (node_id, statuses.into_values().collect()))
        .collect())
}

fn effective_node_status(n: &node::Model) -> String {
    if n.status != "online" {
        return n.status.clone();
    }
    let fresh = n
        .last_seen_at
        .map(|last_seen| {
            (chrono::Utc::now() - last_seen.with_timezone(&chrono::Utc)).num_seconds() <= 90
        })
        .unwrap_or(false);
    if fresh { "online" } else { "offline" }.to_string()
}

pub(crate) fn node_to_json(n: node::Model, core_statuses: Vec<Value>) -> Value {
    let status = effective_node_status(&n);
    json!({
        "id": n.id,
        "name": n.name,
        "hostname": n.hostname,
        "address": n.address,
        "domain": n.domain,
        "cores_available": n.cores_available,
        "labels": n.labels.unwrap_or(json!({})),
        "usage_coefficient": n.usage_coefficient,
        "status": status,
        "parent_id": n.parent_id,
        "last_seen_at": n.last_seen_at,
        "core_statuses": core_statuses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_with_status(
        last_seen_at: Option<chrono::DateTime<chrono::FixedOffset>>,
    ) -> node::Model {
        let now = chrono::Utc::now().fixed_offset();
        node::Model {
            id: Uuid::new_v4(),
            name: "test-node".to_string(),
            hostname: "test-host".to_string(),
            address: "192.0.2.1".to_string(),
            domain: None,
            token_hash: "hash".to_string(),
            cores_available: json!([]),
            labels: None,
            usage_coefficient: 1.0,
            status: "online".to_string(),
            parent_id: None,
            last_seen_at,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn online_status_requires_a_recent_heartbeat() {
        let recent = node_with_status(Some(
            (chrono::Utc::now() - chrono::Duration::seconds(30)).fixed_offset(),
        ));
        assert_eq!(effective_node_status(&recent), "online");

        let stale = node_with_status(Some(
            (chrono::Utc::now() - chrono::Duration::seconds(120)).fixed_offset(),
        ));
        assert_eq!(effective_node_status(&stale), "offline");

        let missing = node_with_status(None);
        assert_eq!(effective_node_status(&missing), "offline");
    }
}
