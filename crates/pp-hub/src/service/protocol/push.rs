use pp_common::{CoreType, PanelResult};
use pp_db::entities::{core_version, node_binding, node_pending_update, protocol_config};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use std::collections::HashSet;
use uuid::Uuid;

/// Content-hash config version (first 16 hex chars of the SHA-256 of the
/// serialized config). Deterministic: identical config -> identical version.
pub fn config_version_of(config_str: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(config_str.as_bytes());
    hex::encode(&digest[..8])
}

/// Push generated core config to the agent running on a node.
/// Returns Ok(()) on successful delivery, or an error describing the failure.
pub async fn push_node_config(
    state: &crate::state::AppState,
    node_id: Uuid,
    core_type: CoreType,
    restart: bool,
    version: Option<String>,
) -> PanelResult<()> {
    validate_node_port_conflicts(&state.db, node_id)
        .await
        .map_err(|e| {
            pp_common::PanelError::Validation(format!("node {} port conflict: {}", node_id, e))
        })?;

    let (config_json, core_version) =
        super::config_gen::generate_node_config(&state.db, node_id, core_type).await?;
    let config_str = serde_json::to_string(&config_json)
        .map_err(|e| pp_common::PanelError::Config(format!("failed to serialize config: {e}")))?;

    let build_id = core_build_id_of(&state.db, core_type, core_version.as_deref()).await;
    let version = version
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| push_version_of(&config_str, &build_id));

    let proto_core = match core_type {
        CoreType::SingBox => pp_proto::CoreType::SingBox,
        CoreType::Mihomo => pp_proto::CoreType::Mihomo,
    };

    let message = pp_proto::HubMessage {
        payload: Some(pp_proto::hub_message::Payload::ConfigPush(
            pp_proto::ConfigPush {
                config_json: config_str,
                target_core: proto_core as i32,
                restart_required: restart,
                config_version: version.clone(),
                core_version: core_version.unwrap_or_default(),
                core_build_id: build_id,
            },
        )),
    };

    state
        .send_to_agent(node_id, message)
        .await
        .map_err(|e| pp_common::PanelError::Internal(format!("failed to push config: {e}")))?;

    state
        .set_agent_config_version(&node_id, &core_type.to_string(), version)
        .await;

    // Clear pending update marker after successful push.
    if let Err(e) = clear_pending(&state.db, node_id, core_type).await {
        tracing::warn!("failed to clear pending update for node {}: {}", node_id, e);
    }

    Ok(())
}

/// Find all node IDs that have an active binding to a given protocol config.
pub async fn nodes_using_config(
    db: &DatabaseConnection,
    config_id: Uuid,
) -> PanelResult<Vec<Uuid>> {
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::ProtocolConfigId.eq(config_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    let node_ids: HashSet<Uuid> = bindings.into_iter().map(|b| b.node_id).collect();
    Ok(node_ids.into_iter().collect())
}

/// Resolve the effective pinned core version from the active core_versions catalog.
///
/// Looks up `core_versions` for a row matching `core_type` with `is_active=true`.
/// sing-box defaults to `v1.13.14` when no active version is found.
/// mihomo returns `None` (= latest upstream) when no active version is found.
pub async fn effective_core_version(
    db: &DatabaseConnection,
    core_type: CoreType,
) -> Option<String> {
    let active = core_version::Entity::find()
        .filter(core_version::Column::CoreType.eq(core_type.to_string()))
        .filter(core_version::Column::IsActive.eq(true))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|v| v.version);

    active.or_else(|| match core_type {
        CoreType::SingBox => Some("v1.13.14".to_string()),
        CoreType::Mihomo => None,
    })
}

pub const UPDATE_TYPE_CONFIG: &str = "config";
pub const UPDATE_TYPE_CORE: &str = "core";

/// Mark (node, core) pairs as having a pending update (upsert by node+core).
pub async fn mark_pending(
    db: &DatabaseConnection,
    node_ids: impl IntoIterator<Item = Uuid>,
    core_type: CoreType,
    update_type: &str,
) -> PanelResult<()> {
    let now = chrono::Utc::now().into();
    let core_type_str = core_type.to_string();

    for node_id in node_ids {
        let existing = node_pending_update::Entity::find()
            .filter(node_pending_update::Column::NodeId.eq(node_id))
            .filter(node_pending_update::Column::CoreType.eq(&core_type_str))
            .one(db)
            .await?;

        if let Some(row) = existing {
            let mut active: node_pending_update::ActiveModel = row.into();
            active.update_type = Set(update_type.to_string());
            active.updated_at = Set(now);
            active.update(db).await?;
        } else {
            let active = node_pending_update::ActiveModel {
                id: Set(Uuid::new_v4()),
                node_id: Set(node_id),
                core_type: Set(core_type_str.clone()),
                update_type: Set(update_type.to_string()),
                updated_at: Set(now),
            };
            active.insert(db).await?;
        }
    }

    Ok(())
}

/// Clear the pending marker after a successful push.
pub async fn clear_pending(
    db: &DatabaseConnection,
    node_id: Uuid,
    core_type: CoreType,
) -> PanelResult<()> {
    node_pending_update::Entity::delete_many()
        .filter(node_pending_update::Column::NodeId.eq(node_id))
        .filter(node_pending_update::Column::CoreType.eq(core_type.to_string()))
        .exec(db)
        .await?;
    Ok(())
}

/// Nodes having at least one active binding whose protocol config targets `core_type`.
pub async fn nodes_with_bindings(
    db: &DatabaseConnection,
    core_type: CoreType,
) -> PanelResult<Vec<Uuid>> {
    let core_type_str = core_type.to_string();
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    let mut node_ids = HashSet::new();
    for binding in bindings {
        let config = protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?;
        if let Some(cfg) = config
            && cfg.core_type == core_type_str
        {
            node_ids.insert(binding.node_id);
        }
    }

    Ok(node_ids.into_iter().collect())
}

/// Build identifier for a pinned core version: the upstream publish time
/// (Unix seconds) recorded in the version catalog. Rolling tags keep the
/// same version string across builds, so this is what actually tells builds
/// apart. Empty when the version is unpinned or has no metadata recorded.
pub async fn core_build_id_of(
    db: &DatabaseConnection,
    core_type: CoreType,
    version: Option<&str>,
) -> String {
    let Some(version) = version.filter(|v| !v.is_empty()) else {
        return String::new();
    };
    core_version::Entity::find()
        .filter(core_version::Column::CoreType.eq(core_type.to_string()))
        .filter(core_version::Column::Version.eq(version))
        .one(db)
        .await
        .ok()
        .flatten()
        .and_then(|v| v.published_at)
        .map(|t| t.timestamp().to_string())
        .unwrap_or_default()
}

/// Config version that also changes when the upstream build of a pinned
/// rolling tag changes, so nodes pick up rebuilt binaries.
pub fn push_version_of(config_str: &str, build_id: &str) -> String {
    if build_id.is_empty() {
        config_version_of(config_str)
    } else {
        config_version_of(&format!("{}#build:{}", config_str, build_id))
    }
}

/// Validate that active bindings on a node do not ask two cores to listen on the
/// same address/port. Returns an error if any overlap is found.
pub async fn validate_node_port_conflicts(
    db: &DatabaseConnection,
    node_id: Uuid,
) -> PanelResult<()> {
    use std::collections::{HashMap, HashSet};

    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::NodeId.eq(node_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    // Map (listen, port) -> set of cores that want it.
    let mut port_cores: HashMap<(String, u16), HashSet<CoreType>> = HashMap::new();

    for binding in bindings {
        let config = match protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?
        {
            Some(c) => c,
            None => continue,
        };

        let mut listen = config.listen_address.clone();
        let mut port = config.listen_port as u16;

        // Apply binding-level overrides for listen/port if present.
        if let Some(overrides) = binding.override_settings
            && let Some(obj) = overrides.as_object()
        {
            if let Some(v) = obj.get("listen_address").and_then(|v| v.as_str()) {
                listen = v.to_string();
            }
            if let Some(v) = obj.get("listen_port").and_then(|v| v.as_u64()) {
                port = v as u16;
            }
        }

        let config_core = super::config_gen::parse_core_type(&config.core_type);
        let cores: Vec<CoreType> = vec![config_core];

        let entry = port_cores.entry((listen.clone(), port)).or_default();
        for c in cores {
            if !entry.insert(c) {
                return Err(pp_common::PanelError::Validation(format!(
                    "port conflict on {}:{}: multiple inbounds target {:?}",
                    listen, port, c
                )));
            }
        }
    }

    Ok(())
}
