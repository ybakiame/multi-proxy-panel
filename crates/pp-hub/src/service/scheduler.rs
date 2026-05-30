use chrono::Datelike;
use pp_common::PanelError;
use pp_db::entities::{client, node_binding, traffic_record};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Set,
};
use serde_json::json;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

/// Periodic background task: checks traffic limits, expiry dates, traffic resets, and pushes updated configs.
pub async fn run_periodic_checks(state: &Arc<AppState>) -> Result<(), PanelError> {
    tracing::info!("running periodic checks");

    // 1. Check and perform traffic resets
    let reset_clients = check_client_traffic_resets(&state.db).await?;
    if !reset_clients.is_empty() {
        tracing::info!("{} clients had traffic reset", reset_clients.len());
    }

    // 2. Check limits and expiration
    let affected_clients = check_client_limits(&state.db).await?;
    if !affected_clients.is_empty() {
        tracing::info!(
            "{} clients affected by limit/expiration",
            affected_clients.len()
        );
        push_updated_configs_for_clients(state, &affected_clients).await?;
    }

    // 3. Also push configs for clients whose traffic was reset (they may have been limited before)
    let mut all_affected = affected_clients;
    all_affected.extend(reset_clients);
    if !all_affected.is_empty() {
        push_updated_configs_for_clients(state, &all_affected).await?;
    }

    Ok(())
}

/// Check all active clients for traffic reset conditions based on `data_limit_reset_strategy`.
/// Returns the set of client IDs whose traffic was reset.
async fn check_client_traffic_resets(
    db: &DatabaseConnection,
) -> Result<HashSet<Uuid>, PanelError> {
    let clients = client::Entity::find()
        .filter(
            Condition::any()
                .add(client::Column::Status.eq("active"))
                .add(client::Column::Status.eq("limited")),
        )
        .all(db)
        .await?;

    let mut reset_clients = HashSet::new();
    let now = chrono::Utc::now();

    for c in clients {
        let strategy = c.data_limit_reset_strategy.as_str();
        if strategy == "no_reset" {
            continue;
        }

        let should_reset = match strategy {
            "daily" => {
                c.last_traffic_reset_time
                    .map(|t| t.date_naive() != now.date_naive())
                    .unwrap_or(true)
            }
            "weekly" => {
                let current_week = now.date_naive().format("%G-%V").to_string();
                c.last_traffic_reset_time
                    .map(|t| t.date_naive().format("%G-%V").to_string() != current_week)
                    .unwrap_or(true)
            }
            "monthly" => {
                c.last_traffic_reset_time
                    .map(|t| {
                        let last_date = t.date_naive();
                        last_date.year() != now.date_naive().year()
                            || last_date.month() != now.date_naive().month()
                    })
                    .unwrap_or(true)
            }
            "yearly" => {
                c.last_traffic_reset_time
                    .map(|t| t.date_naive().year() != now.date_naive().year())
                    .unwrap_or(true)
            }
            _ => false,
        };

        if should_reset {
            let mut active: client::ActiveModel = c.clone().into();
            active.all_time_used_bytes = Set(c.all_time_used_bytes + c.traffic_used_bytes);
            active.traffic_used_bytes = Set(0);
            active.last_traffic_reset_time = Set(Some(now.into()));

            // If client was limited due to traffic, restore to active
            if c.status == "limited" {
                active.status = Set("active".to_string());
            }

            active.update(db).await?;
            reset_clients.insert(c.id);

            log_event(
                db,
                "traffic_reset",
                &format!(
                    "Client {} traffic reset by strategy '{}'. Previous used: {}, all_time: {}",
                    c.id, strategy, c.traffic_used_bytes, c.all_time_used_bytes
                ),
            )
            .await?;

            let _ = crate::service::webhook::trigger_event(
                db,
                "traffic_reset",
                &json!({
                    "client_id": c.id,
                    "strategy": strategy,
                    "previous_used": c.traffic_used_bytes,
                    "all_time": c.all_time_used_bytes + c.traffic_used_bytes,
                }),
            )
            .await;
        }
    }

    Ok(reset_clients)
}

/// Check all active clients for traffic exhaustion and expiration.
/// Returns the set of client IDs whose status was changed.
async fn check_client_limits(db: &DatabaseConnection) -> Result<HashSet<Uuid>, PanelError> {
    let clients = client::Entity::find()
        .filter(client::Column::Status.eq("active"))
        .all(db)
        .await?;

    let mut affected = HashSet::new();

    for c in clients {
        // 1. Check expiration
        if let Some(expiry) = c.expiry_date {
            if expiry < chrono::Utc::now() {
                let mut active: client::ActiveModel = c.clone().into();
                active.status = Set("expired".to_string());
                active.update(db).await?;
                affected.insert(c.id);
                log_event(db, "client_expired", &format!("Client {} expired", c.id)).await?;

                let _ = crate::service::webhook::trigger_event(
                    db,
                    "client_expired",
                    &json!({
                        "client_id": c.id,
                        "name": c.name,
                        "email": c.email,
                    }),
                )
                .await;
                continue;
            }
        }

        // 2. Check traffic limit
        if c.traffic_limit_bytes > 0 {
            let total_used = get_client_traffic_total(db, c.id).await?;
            if total_used >= c.traffic_limit_bytes {
                let mut active: client::ActiveModel = c.clone().into();
                active.status = Set("limited".to_string());
                active.update(db).await?;
                affected.insert(c.id);
                log_event(
                    db,
                    "client_limited",
                    &format!(
                        "Client {} traffic limit exceeded: {} / {}",
                        c.id, total_used, c.traffic_limit_bytes
                    ),
                )
                .await?;

                let _ = crate::service::webhook::trigger_event(
                    db,
                    "client_limited",
                    &json!({
                        "client_id": c.id,
                        "name": c.name,
                        "email": c.email,
                        "total_used": total_used,
                        "traffic_limit": c.traffic_limit_bytes,
                    }),
                )
                .await;
            }
        }
    }

    Ok(affected)
}

/// Calculate total traffic used by a client from traffic_records.
async fn get_client_traffic_total(
    db: &DatabaseConnection,
    client_id: Uuid,
) -> Result<i64, PanelError> {
    let records = traffic_record::Entity::find()
        .filter(traffic_record::Column::ClientId.eq(client_id))
        .all(db)
        .await?;
    let total: i64 = records
        .iter()
        .map(|r| r.upload_bytes + r.download_bytes)
        .sum();
    Ok(total)
}

/// Push updated configs to all nodes that have bindings for the affected clients.
async fn push_updated_configs_for_clients(
    state: &Arc<AppState>,
    _client_ids: &HashSet<Uuid>,
) -> Result<(), PanelError> {
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::IsActive.eq(true))
        .all(&state.db)
        .await?;

    let mut node_ids = HashSet::new();
    for binding in bindings {
        node_ids.insert(binding.node_id);
    }

    for node_id in node_ids {
        if let Ok(config) = crate::service::protocol::generate_node_config(
            &state.db,
            node_id,
            pp_common::CoreType::SingBox,
        )
        .await
        {
            let config_str = serde_json::to_string(&config)
                .map_err(|e| PanelError::Config(format!("serialize config: {}", e)))?;
            let message = pp_proto::HubMessage {
                payload: Some(pp_proto::hub_message::Payload::ConfigPush(
                    pp_proto::ConfigPush {
                        config_json: config_str,
                        target_core: pp_proto::CoreType::SingBox as i32,
                        restart_required: false,
                        config_version: "1".to_string(),
                    },
                )),
            };
            if let Err(e) = state.send_to_agent(node_id, message).await {
                tracing::warn!("failed to push config to node {}: {}", node_id, e);
            }
        }

        if let Ok(config) = crate::service::protocol::generate_node_config(
            &state.db,
            node_id,
            pp_common::CoreType::Xray,
        )
        .await
        {
            let config_str = serde_json::to_string(&config)
                .map_err(|e| PanelError::Config(format!("serialize config: {}", e)))?;
            let message = pp_proto::HubMessage {
                payload: Some(pp_proto::hub_message::Payload::ConfigPush(
                    pp_proto::ConfigPush {
                        config_json: config_str,
                        target_core: pp_proto::CoreType::Xray as i32,
                        restart_required: false,
                        config_version: "1".to_string(),
                    },
                )),
            };
            if let Err(e) = state.send_to_agent(node_id, message).await {
                tracing::warn!("failed to push xray config to node {}: {}", node_id, e);
            }
        }
    }

    Ok(())
}

/// Write an event to system_logs.
async fn log_event(
    db: &DatabaseConnection,
    source: &str,
    message: &str,
) -> Result<(), PanelError> {
    use pp_db::entities::system_log;
    let active = system_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        level: Set("info".to_string()),
        source: Set(source.to_string()),
        message: Set(message.to_string()),
        metadata: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    };
    active.insert(db).await?;
    Ok(())
}
