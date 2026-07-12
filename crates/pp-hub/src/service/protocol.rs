use pp_common::{CoreType, PanelResult};
use pp_config::{BuilderRegistry, InboundConfig};
use pp_db::entities::{
    client, client_group_binding, node_binding, node_binding_group_binding, protocol_config,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use std::collections::HashSet;
use uuid::Uuid;

/// Generate core configuration for a specific node based on its bindings.
/// Returns the config JSON and the effective core binary version (for sing-box
/// only) derived from the active protocol configs.
pub async fn generate_node_config(
    db: &DatabaseConnection,
    node_id: Uuid,
    target_core: CoreType,
) -> PanelResult<(Value, Option<String>)> {
    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::NodeId.eq(node_id))
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    let mut inbounds = Vec::new();

    for binding in bindings {
        let config = protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?
            .ok_or_else(|| {
                pp_common::PanelError::NotFound(format!(
                    "protocol config {} not found",
                    binding.protocol_config_id
                ))
            })?;

        // Check if this config applies to the target core
        let config_core = parse_core_type(&config.core_type);
        if config_core != target_core {
            continue;
        }

        let protocol = parse_protocol_type(&config.protocol_type)?;
        let mut settings = config.settings.clone();

        // Merge override settings from binding, keeping tls_settings separate.
        let override_tls = binding
            .override_settings
            .as_ref()
            .and_then(|o| o.get("tls_settings").cloned());
        if let Some(ref overrides) = binding.override_settings {
            if let Some(obj) = settings.as_object_mut() {
                if let Some(over_obj) = overrides.as_object() {
                    for (k, v) in over_obj {
                        if k == "tls_settings" {
                            continue;
                        }
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        // Inject clients bound to this binding through shared groups.
        inject_binding_clients(db, &binding, &config.protocol_type, &mut settings).await?;

        // Builders read port/listen/tag from settings, so merge InboundConfig fields.
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("port".to_string(), json!(config.listen_port));
            obj.insert("listen".to_string(), json!(config.listen_address.clone()));
            obj.insert(
                "tag".to_string(),
                json!(format!("{}-{}", config.name, config.id)),
            );
        }

        let effective_tls = pp_common::settings_helper::merge_tls_settings(
            config.tls_settings.clone(),
            override_tls,
        );

        inbounds.push(InboundConfig {
            tag: format!("{}-{}", config.name, config.id),
            protocol,
            listen: config.listen_address.clone(),
            port: config.listen_port as u16,
            settings,
            tls: effective_tls,
            sniffing: None,
            core_version: config.core_version.clone(),
        });
    }

    let effective_version = effective_core_version(target_core, &inbounds);

    let registry = BuilderRegistry::default();
    let builder = registry.get(target_core).ok_or_else(|| {
        pp_common::PanelError::Config(format!("no builder registered for {:?}", target_core))
    })?;

    let config = builder.build_full_config(&inbounds)?;
    Ok((config, effective_version))
}

/// Find active clients that share at least one group with a node binding and
/// inject them as a `clients` array into the protocol settings.
async fn inject_binding_clients(
    db: &DatabaseConnection,
    binding: &node_binding::Model,
    protocol_type: &str,
    settings: &mut Value,
) -> PanelResult<()> {
    let binding_group_ids = node_binding_group_binding::Entity::find()
        .filter(node_binding_group_binding::Column::NodeBindingId.eq(binding.id))
        .all(db)
        .await?
        .into_iter()
        .map(|g| g.group_id)
        .collect::<Vec<_>>();

    if binding_group_ids.is_empty() {
        // No groups on the binding means no clients are authorized.
        return Ok(());
    }

    let group_set: HashSet<Uuid> = binding_group_ids.into_iter().collect();

    // Find active clients whose group memberships overlap with the binding groups.
    let client_bindings = client_group_binding::Entity::find()
        .filter(client_group_binding::Column::GroupId.is_in(group_set.iter().cloned()))
        .all(db)
        .await?;

    let client_ids: HashSet<Uuid> = client_bindings.into_iter().map(|b| b.client_id).collect();

    if client_ids.is_empty() {
        return Ok(());
    }

    let clients = client::Entity::find()
        .filter(client::Column::Id.is_in(client_ids.iter().cloned()))
        .filter(client::Column::Status.eq("active"))
        .all(db)
        .await?;

    let clients_json: Vec<Value> = clients
        .iter()
        .map(|c| client_to_protocol_entry(c, protocol_type))
        .collect();

    if let Some(obj) = settings.as_object_mut() {
        obj.insert("clients".to_string(), json!(clients_json));
    }

    Ok(())
}

/// Map a client to the protocol-specific client entry expected by pp-config builders.
fn client_to_protocol_entry(client: &client::Model, protocol_type: &str) -> Value {
    let email = client.email.as_ref().unwrap_or(&client.name);
    match protocol_type {
        pt if pt.starts_with("vless") => {
            let flow = if pt == "vless_reality" {
                "xtls-rprx-vision"
            } else {
                ""
            };
            let mut obj = json!({
                "id": client.id.to_string(),
                "email": email,
                "flow": flow,
            });
            if let Some(limit) = client.max_devices {
                if limit > 0 {
                    if let Some(map) = obj.as_object_mut() {
                        map.insert("limitIp".to_string(), json!(limit));
                    }
                }
            }
            obj
        }
        "hysteria2" | "anytls" => json!({
            "name": email,
            "password": client.id.to_string(),
        }),
        _ => json!({"id": client.id.to_string()}),
    }
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

    let (config_json, core_version) = generate_node_config(&state.db, node_id, core_type).await?;
    let config_str = serde_json::to_string(&config_json)
        .map_err(|e| pp_common::PanelError::Config(format!("failed to serialize config: {e}")))?;

    let proto_core = match core_type {
        CoreType::Xray => pp_proto::CoreType::Xray,
        CoreType::SingBox => pp_proto::CoreType::SingBox,
    };

    let message = pp_proto::HubMessage {
        payload: Some(pp_proto::hub_message::Payload::ConfigPush(
            pp_proto::ConfigPush {
                config_json: config_str,
                target_core: proto_core as i32,
                restart_required: restart,
                config_version: version.unwrap_or_else(|| "1".to_string()),
                core_version: core_version.unwrap_or_default(),
            },
        )),
    };

    state
        .send_to_agent(node_id, message)
        .await
        .map_err(|e| pp_common::PanelError::Internal(format!("failed to push config: {e}")))?;

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
/// For sing-box, returns the highest explicitly requested version or a stable
/// default of v1.13.14. If you need the new gRPC API service, set a protocol
/// config's core_version to a 1.14.0 alpha tag such as `v1.14.0-alpha.43`.
fn effective_core_version(core_type: CoreType, inbounds: &[InboundConfig]) -> Option<String> {
    if core_type != CoreType::SingBox {
        return None;
    }

    let requested: Vec<&str> = inbounds
        .iter()
        .filter_map(|i| i.core_version.as_deref())
        .filter(|v| !v.is_empty())
        .collect();

    if requested.is_empty() {
        return Some("v1.13.14".to_string());
    }

    requested
        .into_iter()
        .max_by(|a, b| compare_versions(a, b))
        .map(|v| v.to_string())
}

/// Simple semver-like comparison. Returns `Ordering` for two version strings.
/// Pre-release segments (e.g. `-beta.5`) are treated as lower than release.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    fn parse(v: &str) -> Vec<u32> {
        v.split(|c: char| !c.is_ascii_digit())
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect()
    }
    parse(a).cmp(&parse(b))
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
        if let Some(overrides) = binding.override_settings {
            if let Some(obj) = overrides.as_object() {
                if let Some(v) = obj.get("listen_address").and_then(|v| v.as_str()) {
                    listen = v.to_string();
                }
                if let Some(v) = obj.get("listen_port").and_then(|v| v.as_u64()) {
                    port = v as u16;
                }
            }
        }

        let config_core = parse_core_type(&config.core_type);
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

fn parse_core_type(s: &str) -> CoreType {
    match s {
        "xray" => CoreType::Xray,
        "sing-box" | "singbox" => CoreType::SingBox,
        _ => CoreType::SingBox,
    }
}

fn parse_protocol_type(s: &str) -> PanelResult<pp_common::ProtocolType> {
    use pp_common::ProtocolType;
    match s {
        "vless_reality" => Ok(ProtocolType::VlessReality),
        "vless_xhttp" => Ok(ProtocolType::VlessXhttp),
        "hysteria2" => Ok(ProtocolType::Hysteria2),
        "anytls" => Ok(ProtocolType::Anytls),
        _ => Err(pp_common::PanelError::Validation(format!(
            "unknown protocol type: {}",
            s
        ))),
    }
}
