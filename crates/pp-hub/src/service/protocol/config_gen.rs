use pp_common::{CoreType, PanelResult};
use pp_config::{BuilderRegistry, InboundConfig};
use pp_db::entities::{
    certificate, client, client_group_binding, node_binding, node_binding_group_binding,
    protocol_config,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde_json::{Value, json};
use std::collections::HashSet;
use uuid::Uuid;

/// Generate core configuration for a specific node based on its bindings.
/// Returns the config JSON and the effective core binary version derived from
/// the active core_versions catalog.
pub async fn generate_node_config(
    db: &DatabaseConnection,
    node_id: Uuid,
    target_core: CoreType,
) -> PanelResult<(Value, Option<String>)> {
    // Resolve the active core version once, before the inbounds loop.
    let effective_version = super::effective_core_version(db, target_core).await;

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
        if let Some(ref overrides) = binding.override_settings
            && let Some(obj) = settings.as_object_mut()
            && let Some(over_obj) = overrides.as_object()
        {
            for (k, v) in over_obj {
                if k == "tls_settings" {
                    continue;
                }
                obj.insert(k.clone(), v.clone());
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
        let effective_tls = resolve_managed_cert_tls(db, node_id, effective_tls).await?;

        inbounds.push(InboundConfig {
            tag: format!("{}-{}", config.name, config.id),
            protocol,
            listen: config.listen_address.clone(),
            port: config.listen_port as u16,
            settings,
            tls: effective_tls,
            sniffing: None,
            core_version: effective_version.clone(),
        });
    }

    let registry = BuilderRegistry::default();
    let builder = registry.get(target_core).ok_or_else(|| {
        pp_common::PanelError::Config(format!("no builder registered for {:?}", target_core))
    })?;

    let mut config = builder.build_full_config(&inbounds)?;
    super::relay::apply_relay_rules(db, node_id, target_core, &mut config).await?;

    // Validate generated sing-box configs against the official JSON Schema.
    if target_core == CoreType::SingBox {
        pp_config::validate_singbox_config(&config)?;
    }

    Ok((config, effective_version))
}

/// Translate a managed-certificate TLS reference (`{"cert_id": ...}`) into
/// the agent-side unified layout (`certs/<domain>.{crt,key}`). The
/// certificate must belong to the node the config is generated for.
pub async fn resolve_managed_cert_tls(
    db: &DatabaseConnection,
    node_id: Uuid,
    tls: Option<Value>,
) -> PanelResult<Option<Value>> {
    let Some(tls) = tls else {
        return Ok(None);
    };
    let Some(cert_id_raw) = tls.get("cert_id").and_then(|v| v.as_str()) else {
        return Ok(Some(tls));
    };

    let cert_id = Uuid::parse_str(cert_id_raw)
        .map_err(|_| pp_common::PanelError::Validation("invalid cert_id in tls_settings".into()))?;
    let cert = certificate::Entity::find_by_id(cert_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            pp_common::PanelError::NotFound(format!("certificate {} not found", cert_id))
        })?;
    if cert.node_id != node_id {
        return Err(pp_common::PanelError::Validation(format!(
            "certificate {} ({}) does not belong to this node",
            cert_id, cert.domain
        )));
    }

    Ok(Some(json!({ "managed_domain": cert.domain })))
}

/// Find active clients that share at least one group with a node binding and
/// inject them as a `clients` array into the protocol settings.
pub async fn inject_binding_clients(
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
///
/// The injected identifier (vless `email`, password-protocol `name`) is what the
/// core reports back as the connection user, so it must stay resolvable by the
/// hub: fall back to the client UUID when no email is set.
pub fn client_to_protocol_entry(client: &client::Model, protocol_type: &str) -> Value {
    let fallback = client.id.to_string();
    let email = client.email.as_ref().unwrap_or(&fallback);
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
            if let Some(limit) = client.max_devices
                && limit > 0
                && let Some(map) = obj.as_object_mut()
            {
                map.insert("limitIp".to_string(), json!(limit));
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

pub fn parse_core_type(s: &str) -> CoreType {
    match s {
        "sing-box" | "singbox" => CoreType::SingBox,
        "mihomo" => CoreType::Mihomo,
        _ => CoreType::SingBox,
    }
}

pub fn parse_protocol_type(s: &str) -> PanelResult<pp_common::ProtocolType> {
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
