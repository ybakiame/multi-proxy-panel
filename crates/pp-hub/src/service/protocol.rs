use pp_common::{CoreType, PanelResult};
use pp_config::{BuilderRegistry, InboundConfig};
use pp_db::entities::{node_binding, protocol_config};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
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
        if config_core != CoreType::Both && config_core != target_core {
            continue;
        }

        let protocol = parse_protocol_type(&config.protocol_type)?;
        let mut settings = config.settings.clone();

        // Merge override settings from binding
        if let Some(overrides) = binding.override_settings {
            if let Some(obj) = settings.as_object_mut() {
                if let Some(over_obj) = overrides.as_object() {
                    for (k, v) in over_obj {
                        obj.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        inbounds.push(InboundConfig {
            tag: format!("{}-{}", config.name, config.id),
            protocol,
            listen: config.listen_address.clone(),
            port: config.listen_port as u16,
            settings,
            tls: config.tls_settings.clone(),
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

/// Determine the effective core binary version for a set of inbounds.
/// For sing-box, returns the highest explicitly requested version or a default
/// of 1.14.0 so that new server deployments use the modern API service.
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
        return Some("1.14.0".to_string());
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
        let cores: Vec<CoreType> = match config_core {
            CoreType::Both => vec![CoreType::Xray, CoreType::SingBox],
            c => vec![c],
        };

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
        _ => CoreType::Both,
    }
}

fn parse_protocol_type(s: &str) -> PanelResult<pp_common::ProtocolType> {
    use pp_common::ProtocolType;
    match s {
        "vless_reality" => Ok(ProtocolType::VlessReality),
        "vless_vision" => Ok(ProtocolType::VlessVision),
        "vless_xhttp" => Ok(ProtocolType::VlessXhttp),
        "vmess" => Ok(ProtocolType::Vmess),
        "trojan" => Ok(ProtocolType::Trojan),
        "shadowsocks2022" => Ok(ProtocolType::Shadowsocks2022),
        "hysteria2" => Ok(ProtocolType::Hysteria2),
        "tuic" | "tuic_v5" => Ok(ProtocolType::TuicV5),
        "anytls" => Ok(ProtocolType::Anytls),
        _ => Err(pp_common::PanelError::Validation(format!(
            "unknown protocol type: {}",
            s
        ))),
    }
}
