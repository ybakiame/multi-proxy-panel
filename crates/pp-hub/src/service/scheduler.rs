use chrono::{Datelike, Duration};
use pp_common::PanelError;
use pp_db::entities::{client, client_online_session, node_binding, protocol_config, system_log};
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

    // 3. Check on-hold timeouts (expire clients who never connected within the hold window)
    let expired_hold = check_on_hold_timeouts(&state.db).await?;
    if !expired_hold.is_empty() {
        tracing::info!("{} on-hold clients expired (timeout)", expired_hold.len());
    }

    // 4. Also push configs for clients whose traffic was reset or on-hold expired
    let mut all_affected = affected_clients;
    all_affected.extend(reset_clients);
    all_affected.extend(expired_hold);
    if !all_affected.is_empty() {
        push_updated_configs_for_clients(state, &all_affected).await?;
    }

    // 5. Cleanup old records
    cleanup_old_records(&state.db).await?;

    // 6. Stop cores that have no active bindings on their node (leftover from
    //    deleted protocol configs or previous pushes).
    if let Err(e) = reconcile_orphan_cores(state).await {
        tracing::warn!("reconcile orphan cores failed: {}", e);
    }

    Ok(())
}

/// Cleanup old system logs and stale online sessions.
async fn cleanup_old_records(db: &DatabaseConnection) -> Result<(), PanelError> {
    // Delete system logs older than 30 days
    let cutoff_logs = chrono::Utc::now() - Duration::days(30);
    let deleted_logs = system_log::Entity::delete_many()
        .filter(system_log::Column::CreatedAt.lt(cutoff_logs))
        .exec(db)
        .await?;
    if deleted_logs.rows_affected > 0 {
        tracing::info!(
            "cleaned up {} old system log records",
            deleted_logs.rows_affected
        );
    }

    // Delete online sessions older than 10 minutes (stale sessions)
    let cutoff_sessions = chrono::Utc::now() - Duration::minutes(10);
    let deleted_sessions = client_online_session::Entity::delete_many()
        .filter(client_online_session::Column::LastActiveAt.lt(cutoff_sessions))
        .exec(db)
        .await?;
    if deleted_sessions.rows_affected > 0 {
        tracing::info!(
            "cleaned up {} stale online sessions",
            deleted_sessions.rows_affected
        );
    }

    Ok(())
}

/// Check all active clients for traffic reset conditions based on `data_limit_reset_strategy`.
/// Returns the set of client IDs whose traffic was reset.
async fn check_client_traffic_resets(db: &DatabaseConnection) -> Result<HashSet<Uuid>, PanelError> {
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
            "daily" => c
                .last_traffic_reset_time
                .map(|t| t.date_naive() != now.date_naive())
                .unwrap_or(true),
            "weekly" => {
                let current_week = now.date_naive().format("%G-%V").to_string();
                c.last_traffic_reset_time
                    .map(|t| t.date_naive().format("%G-%V").to_string() != current_week)
                    .unwrap_or(true)
            }
            "monthly" => c
                .last_traffic_reset_time
                .map(|t| {
                    let last_date = t.date_naive();
                    last_date.year() != now.date_naive().year()
                        || last_date.month() != now.date_naive().month()
                })
                .unwrap_or(true),
            "yearly" => c
                .last_traffic_reset_time
                .map(|t| t.date_naive().year() != now.date_naive().year())
                .unwrap_or(true),
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
        if let Some(expiry) = c.expiry_date
            && expiry < chrono::Utc::now()
        {
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

        // 2. Check traffic limit (traffic_used_bytes is maintained by agent traffic reports)
        if c.traffic_limit_bytes > 0 {
            let total_used = c.traffic_used_bytes;
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

/// Check on-hold clients whose on_hold_timeout has passed and mark them expired.
/// Returns the set of client IDs that were expired.
async fn check_on_hold_timeouts(db: &DatabaseConnection) -> Result<HashSet<Uuid>, PanelError> {
    let clients = client::Entity::find()
        .filter(client::Column::Status.eq("on_hold"))
        .all(db)
        .await?;

    let mut expired = HashSet::new();
    let now = chrono::Utc::now();

    for c in clients {
        if let Some(timeout) = c.on_hold_timeout
            && timeout < now
        {
            let mut active: client::ActiveModel = c.clone().into();
            active.status = Set("expired".to_string());
            active.update(db).await?;
            expired.insert(c.id);
            log_event(
                db,
                "on_hold_expired",
                &format!("Client {} on-hold timeout expired", c.id),
            )
            .await?;

            let _ = crate::service::webhook::trigger_event(
                db,
                "on_hold_expired",
                &json!({
                    "client_id": c.id,
                    "name": c.name,
                    "email": c.email,
                    "timeout": timeout.to_rfc3339(),
                }),
            )
            .await;
        }
    }

    Ok(expired)
}

/// Push updated configs to all nodes that have active bindings.
///
/// Note: client_ids is not used for filtering because config push updates
/// inbound configurations (protocol/port/TLS), not client credentials.
/// Client access control is handled at subscription generation time based
/// on client status (active/limited/expired).
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
        push_config_for_core(state, node_id, pp_common::CoreType::SingBox).await;
        push_config_for_core(state, node_id, pp_common::CoreType::Mihomo).await;
    }

    Ok(())
}

/// Generate and push config for a specific core type, but only if the node
/// has active bindings for that core type (avoids pushing empty configs).
async fn push_config_for_core(
    state: &Arc<AppState>,
    node_id: Uuid,
    core_type: pp_common::CoreType,
) {
    // First check if there are any bindings that target this core type on this node
    let has_bindings = match check_bindings_for_core(&state.db, node_id, core_type).await {
        Ok(true) => true,
        _ => return,
    };

    if !has_bindings {
        return;
    }

    match crate::service::protocol::generate_node_config(&state.db, node_id, core_type).await {
        Ok((config, core_version)) => {
            // Verify the generated config has inbounds (the core needs at least one)
            let has_inbounds = config
                .get("inbounds")
                .and_then(|v| v.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false);
            if !has_inbounds {
                return;
            }

            let config_str = match serde_json::to_string(&config) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("failed to serialize config for node {}: {}", node_id, e);
                    return;
                }
            };

            let build_id = crate::service::protocol::core_build_id_of(
                &state.db,
                core_type,
                core_version.as_deref(),
            )
            .await;
            let version = crate::service::protocol::push_version_of(&config_str, &build_id);
            let core_name = core_type.to_string();
            if state
                .agent_config_version(&node_id, &core_name)
                .await
                .as_deref()
                == Some(version.as_str())
            {
                return;
            }

            let proto_core = match core_type {
                pp_common::CoreType::SingBox => pp_proto::CoreType::SingBox,
                pp_common::CoreType::Mihomo => pp_proto::CoreType::Mihomo,
            };

            let message = pp_proto::HubMessage {
                payload: Some(pp_proto::hub_message::Payload::ConfigPush(
                    pp_proto::ConfigPush {
                        config_json: config_str,
                        target_core: proto_core as i32,
                        restart_required: false,
                        config_version: version.clone(),
                        core_version: core_version.unwrap_or_default(),
                        core_build_id: build_id,
                    },
                )),
            };

            match state.send_to_agent(node_id, message).await {
                Ok(()) => {
                    state
                        .set_agent_config_version(&node_id, &core_name, version)
                        .await;
                    if let Err(e) =
                        crate::service::protocol::clear_pending(&state.db, node_id, core_type).await
                    {
                        tracing::warn!("failed to clear pending for node {}: {}", node_id, e);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "failed to push {:?} config to node {}: {}",
                        core_type,
                        node_id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                "failed to generate {:?} config for node {}: {}",
                core_type,
                node_id,
                e
            );
        }
    }
}

/// Check if a node has active bindings for a specific core type.
async fn check_bindings_for_core(
    db: &DatabaseConnection,
    node_id: Uuid,
    core_type: pp_common::CoreType,
) -> Result<bool, PanelError> {
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::NodeId.eq(node_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    for binding in bindings {
        let config = protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?
            .map(|c| c.core_type)
            .unwrap_or_default();

        let binding_core = match config.as_str() {
            "sing-box" | "singbox" => pp_common::CoreType::SingBox,
            "mihomo" => pp_common::CoreType::Mihomo,
            _ => pp_common::CoreType::SingBox,
        };

        if binding_core == core_type {
            return Ok(true);
        }
    }

    Ok(false)
}

/// Write an event to system_logs.
async fn log_event(db: &DatabaseConnection, source: &str, message: &str) -> Result<(), PanelError> {
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

/// For every connected agent node, stop cores that have no active bindings
/// but still carry a recorded config_version (i.e. they were pushed earlier
/// and now the last binding was removed).
async fn reconcile_orphan_cores(state: &Arc<AppState>) -> Result<(), PanelError> {
    let agent_ids: Vec<Uuid> = state.agents.read().await.keys().copied().collect();

    for node_id in agent_ids {
        for core_type in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
            let has_bindings = check_bindings_for_core(&state.db, node_id, core_type)
                .await
                .unwrap_or(true);
            if has_bindings {
                continue;
            }

            let core_name = core_type.to_string();
            if state
                .agent_config_version(&node_id, &core_name)
                .await
                .is_none()
            {
                continue;
            }

            tracing::info!(
                "stopping orphan {:?} on node {} (no active bindings)",
                core_type,
                node_id
            );

            let proto_core = match core_type {
                pp_common::CoreType::SingBox => pp_proto::CoreType::SingBox,
                pp_common::CoreType::Mihomo => pp_proto::CoreType::Mihomo,
            };

            let message = pp_proto::HubMessage {
                payload: Some(pp_proto::hub_message::Payload::CoreCmd(
                    pp_proto::CoreCommand {
                        command: Some(pp_proto::core_command::Command::Stop(pp_proto::CoreStop {
                            core_type: proto_core as i32,
                        })),
                    },
                )),
            };

            if state.send_to_agent(node_id, message).await.is_ok() {
                state
                    .set_agent_config_version(&node_id, &core_name, String::new())
                    .await;
            }
        }
    }

    Ok(())
}
