//! Persist authenticated Agent log batches.
use crate::state::AppState;
use pp_db::entities::agent_log;
use pp_proto::LogBatch;
use sea_orm::{ActiveModelTrait, Set};
use uuid::Uuid;

pub(super) async fn handle_logs(
    state: &AppState,
    agent_id: Uuid,
    logs: LogBatch,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for entry in logs.entries.into_iter().take(1000) {
        let active = agent_log::ActiveModel {
            id: Set(Uuid::new_v4()),
            node_id: Set(agent_id),
            level: Set(entry.level.to_ascii_lowercase()),
            target: Set(pp_common::logging::sanitize_log_field(
                "target",
                &entry.target,
            )),
            message: Set(pp_common::logging::sanitize_log_text(&entry.message)),
            fields: Set(Some(serde_json::json!(
                entry
                    .fields
                    .into_iter()
                    .take(32)
                    .map(|(key, value)| {
                        let value = pp_common::logging::sanitize_log_field(&key, &value);
                        (key, value)
                    })
                    .collect::<std::collections::HashMap<_, _>>()
            ))),
            created_at: Set(chrono::DateTime::from_timestamp(entry.timestamp, 0)
                .unwrap_or_else(chrono::Utc::now)
                .into()),
        };
        if active.insert(&state.db).await.is_err() {
            tracing::warn!("failed to insert agent log");
        }
    }

    Ok(())
}
