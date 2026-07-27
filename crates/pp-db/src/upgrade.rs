//! Version-aware data upgrades.
//!
//! Sea-ORM migrations handle schema changes incrementally on every startup.
//! Logical/data upgrades (e.g. purging rows of a dropped feature) are instead
//! gated on the application version recorded in `system_meta`: each upgrade
//! step runs exactly once, when the database is opened by a binary whose
//! version crosses the step's `introduced_in` version — never unconditionally
//! on every boot.

use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};
use tracing::{info, warn};

use crate::entities::{
    agent_log, core_version, node, node_binding, node_binding_group_binding, protocol_config,
    subscription_template, system_meta,
};

const SINGBOX_BUILTIN_TEMPLATE: &str = include_str!("templates/singbox_builtin.json");

const VERSION_KEY: &str = "app_version";

type UpgradeFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DbErr>> + Send + 'a>>;

/// Version assumed for databases created before version tracking existed.
/// Ensures the upgrade steps of the first versioned release still run once
/// on legacy deployments.
pub const LEGACY_BASELINE_VERSION: &str = "0.1.0";

struct UpgradeStep {
    /// Release that introduced this step.
    introduced_in: &'static str,
    name: &'static str,
    run: for<'a> fn(&'a DatabaseConnection) -> UpgradeFuture<'a>,
}

const UPGRADE_STEPS: &[UpgradeStep] = &[
    UpgradeStep {
        introduced_in: "0.2.0",
        name: "purge_xray_data",
        run: |conn| Box::pin(purge_xray_data(conn)),
    },
    UpgradeStep {
        introduced_in: "0.3.1",
        name: "refresh_singbox_builtin_template",
        run: |conn| Box::pin(refresh_singbox_builtin_template(conn)),
    },
    UpgradeStep {
        introduced_in: "0.3.2",
        name: "refresh_singbox_builtin_template_v2",
        run: |conn| Box::pin(refresh_singbox_builtin_template(conn)),
    },
    UpgradeStep {
        introduced_in: "0.3.3",
        name: "refresh_singbox_builtin_template_v3",
        run: |conn| Box::pin(refresh_singbox_builtin_template(conn)),
    },
];

/// Run every upgrade step whose version falls in `(stored_version, app_version]`,
/// then record `app_version` in `system_meta`.
pub async fn run_versioned_upgrades(
    conn: &DatabaseConnection,
    app_version: &str,
) -> Result<(), DbErr> {
    let stored = get_system_version(conn)
        .await?
        .unwrap_or_else(|| LEGACY_BASELINE_VERSION.to_string());

    for step in UPGRADE_STEPS {
        if compare_versions(stored.as_str(), step.introduced_in) == std::cmp::Ordering::Less
            && compare_versions(step.introduced_in, app_version) != std::cmp::Ordering::Greater
        {
            info!(
                "running upgrade step {} (v{})",
                step.name, step.introduced_in
            );
            (step.run)(conn).await.map_err(|e| {
                warn!("upgrade step {} failed: {}", step.name, e);
                e
            })?;
            set_system_version(conn, step.introduced_in).await?;
        }
    }

    let current = get_system_version(conn).await?;
    if current.as_deref() != Some(app_version) {
        set_system_version(conn, app_version).await?;
    }

    Ok(())
}

/// Read the recorded application version, if any.
pub async fn get_system_version(conn: &DatabaseConnection) -> Result<Option<String>, DbErr> {
    let row = system_meta::Entity::find_by_id(VERSION_KEY)
        .one(conn)
        .await?;
    Ok(row.map(|r| r.value))
}

async fn set_system_version(conn: &DatabaseConnection, version: &str) -> Result<(), DbErr> {
    let now: sea_orm::prelude::DateTimeWithTimeZone = chrono::Utc::now().into();
    let existing = system_meta::Entity::find_by_id(VERSION_KEY)
        .one(conn)
        .await?;
    if let Some(model) = existing {
        let mut active: system_meta::ActiveModel = model.into();
        active.value = Set(version.to_string());
        active.updated_at = Set(now);
        active.update(conn).await?;
    } else {
        system_meta::ActiveModel {
            key: Set(VERSION_KEY.to_string()),
            value: Set(version.to_string()),
            updated_at: Set(now),
        }
        .insert(conn)
        .await?;
    }
    Ok(())
}

/// Compare dotted version strings segment by segment (`1.10.0` > `1.9.0`).
/// Pre-release suffixes (`-alpha.3`) sort before the plain release.
pub fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse(v: &str) -> (Vec<u64>, bool) {
        let v = v.trim_start_matches('v');
        let (base, pre) = match v.split_once('-') {
            Some((base, _)) => (base, true),
            None => (v, false),
        };
        let segments = base
            .split('.')
            .filter_map(|s| s.parse::<u64>().ok())
            .collect();
        (segments, pre)
    }
    let (sa, pa) = parse(a);
    let (sb, pb) = parse(b);
    sa.cmp(&sb).then(pa.cmp(&pb).reverse())
}

/// 0.2.0 — remove every trace of xray from the database: bindings pointing
/// at xray protocol configs, the configs themselves, tracked core versions,
/// and stale `xray` entries in nodes' `cores_available` lists.
async fn purge_xray_data(conn: &DatabaseConnection) -> Result<(), DbErr> {
    let xray_configs = protocol_config::Entity::find()
        .filter(protocol_config::Column::CoreType.eq("xray"))
        .all(conn)
        .await?;
    let config_ids: Vec<uuid::Uuid> = xray_configs.iter().map(|c| c.id).collect();

    if !config_ids.is_empty() {
        let bindings = node_binding::Entity::find()
            .filter(node_binding::Column::ProtocolConfigId.is_in(config_ids.clone()))
            .all(conn)
            .await?;
        let binding_ids: Vec<uuid::Uuid> = bindings.iter().map(|b| b.id).collect();

        if !binding_ids.is_empty() {
            let removed = node_binding_group_binding::Entity::delete_many()
                .filter(
                    node_binding_group_binding::Column::NodeBindingId.is_in(binding_ids.clone()),
                )
                .exec(conn)
                .await?;
            info!("purged {} xray binding group links", removed.rows_affected);

            let removed = node_binding::Entity::delete_many()
                .filter(node_binding::Column::Id.is_in(binding_ids))
                .exec(conn)
                .await?;
            info!("purged {} xray node bindings", removed.rows_affected);
        }

        let removed = protocol_config::Entity::delete_many()
            .filter(protocol_config::Column::Id.is_in(config_ids))
            .exec(conn)
            .await?;
        info!("purged {} xray protocol configs", removed.rows_affected);
    }

    let removed = core_version::Entity::delete_many()
        .filter(core_version::Column::CoreType.eq("xray"))
        .exec(conn)
        .await?;
    info!("purged {} xray core versions", removed.rows_affected);

    // Core-status history for xray would otherwise linger as "running"
    // forever now that agents no longer report it.
    let removed = agent_log::Entity::delete_many()
        .filter(agent_log::Column::Target.eq("core-xray"))
        .exec(conn)
        .await?;
    info!("purged {} xray core-status logs", removed.rows_affected);

    // Drop "xray" from nodes.cores_available (a JSON string array).
    let nodes = node::Entity::find().all(conn).await?;
    for n in nodes {
        let Some(cores) = n.cores_available.as_array() else {
            continue;
        };
        let filtered: Vec<serde_json::Value> = cores
            .iter()
            .filter(|c| c.as_str() != Some("xray"))
            .cloned()
            .collect();
        if filtered.len() != cores.len() {
            let mut active: node::ActiveModel = n.into();
            active.cores_available = Set(serde_json::Value::Array(filtered));
            active.update(conn).await?;
        }
    }

    Ok(())
}

/// 0.3.1 — replace the built-in sing-box subscription template with the
/// 1.12+/1.14-compatible format (legacy DNS/geosite fields were removed
/// upstream). Only rows flagged is_builtin are touched; user-customized
/// templates are left alone.
async fn refresh_singbox_builtin_template(conn: &DatabaseConnection) -> Result<(), DbErr> {
    let templates = subscription_template::Entity::find()
        .filter(subscription_template::Column::IsBuiltin.eq(true))
        .filter(subscription_template::Column::Format.eq("sing-box"))
        .all(conn)
        .await?;
    for t in templates {
        let mut active: subscription_template::ActiveModel = t.into();
        active.base_config = Set(Some(SINGBOX_BUILTIN_TEMPLATE.to_string()));
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(conn).await?;
        info!("refreshed builtin sing-box subscription template");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_versions_orders_dotted_segments() {
        assert_eq!(compare_versions("0.1.0", "0.2.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("0.10.0", "0.9.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("0.2.0", "0.2.0"),
            std::cmp::Ordering::Equal
        );
        assert_eq!(
            compare_versions("1.14.0-alpha.3", "1.14.0"),
            std::cmp::Ordering::Less
        );
    }
}
