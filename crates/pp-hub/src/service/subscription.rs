//! Subscription business logic.
//!
//! Subscription generation and proxy node building logic lives in
//! `crate::routes::subscription` (route handlers) and
//! `pp-subscription` crate (format generators).
//!
//! This module is reserved for service-layer subscription operations
//! that are shared across HTTP and gRPC handlers.

use pp_common::PanelResult;
use pp_db::entities::{client, client_group_binding, node, node_binding, protocol_config};
use pp_subscription::ProxyNode;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

/// Fetch group IDs assigned to a client.
#[allow(dead_code)]
pub async fn get_client_group_ids(
    db: &DatabaseConnection,
    client_id: Uuid,
) -> PanelResult<Vec<Uuid>> {
    let bindings = client_group_binding::Entity::find()
        .filter(client_group_binding::Column::ClientId.eq(client_id))
        .all(db)
        .await?;
    Ok(bindings.into_iter().map(|b| b.group_id).collect())
}

/// Build proxy nodes for subscription generation.
#[allow(dead_code)]
pub async fn build_proxy_nodes(
    db: &DatabaseConnection,
    client_model: &client::Model,
) -> PanelResult<Vec<ProxyNode>> {
    if client_model.status == "limited" || client_model.status == "expired" {
        return Ok(Vec::new());
    }

    let _client_group_ids = get_client_group_ids(db, client_model.id).await?;

    let bindings = node_binding::Entity::find()
        .filter(node_binding::Column::IsActive.eq(true))
        .all(db)
        .await?;

    let mut nodes = Vec::new();

    for binding in bindings {
        let config = protocol_config::Entity::find_by_id(binding.protocol_config_id)
            .one(db)
            .await?;
        let node_model = node::Entity::find_by_id(binding.node_id).one(db).await?;

        if let (Some(cfg), Some(node)) = (config, node_model) {
            let protocol = match cfg.protocol_type.as_str() {
                "vless_reality" => pp_common::ProtocolType::VlessReality,
                "vless_xhttp" => pp_common::ProtocolType::VlessXhttp,
                "hysteria2" => pp_common::ProtocolType::Hysteria2,
                "anytls" => pp_common::ProtocolType::Anytls,
                _ => continue,
            };

            let tls = pp_common::settings_helper::merge_tls_settings(
                cfg.tls_settings.clone(),
                binding
                    .override_settings
                    .as_ref()
                    .and_then(|o| o.get("tls_settings").cloned()),
            );

            nodes.push(ProxyNode {
                name: format!("{}-{}", node.name, cfg.name),
                protocol,
                server: node.domain.clone().unwrap_or_else(|| node.address.clone()),
                port: cfg.listen_port as u16,
                settings: cfg.settings.clone(),
                tls,
            });
        }
    }

    Ok(nodes)
}
