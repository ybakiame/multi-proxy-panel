use pp_common::{CoreType, PanelResult};
use pp_config::{BuilderRegistry, ConfigBuilder, InboundConfig};
use pp_db::entities::{node_binding, protocol_config};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::Value;
use uuid::Uuid;

/// Generate core configuration for a specific node based on its bindings.
pub async fn generate_node_config(
    db: &DatabaseConnection,
    node_id: Uuid,
    target_core: CoreType,
) -> PanelResult<Value> {
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
            .ok_or_else(|| pp_common::PanelError::NotFound(format!(
                "protocol config {} not found",
                binding.protocol_config_id
            )))?;

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
        });
    }

    let registry = BuilderRegistry::default();
    let builder = registry
        .get(target_core)
        .ok_or_else(|| pp_common::PanelError::Config(format!(
            "no builder registered for {:?}",
            target_core
        )))?;

    builder.build_full_config(&inbounds)
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
