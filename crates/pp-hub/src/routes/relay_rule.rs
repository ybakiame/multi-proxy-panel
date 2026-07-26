use axum::{
    extract::{Path, State},
    response::Json,
};
use pp_db::entities::{
    client, client_group_binding, node, node_binding, node_binding_group_binding, protocol_config,
    relay_rule,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

/// Built-in community rule sets, dual-format per core.
/// (name, sing-box url, sing-box format, mihomo url, mihomo behavior)
pub const RULE_SET_LIBRARY: &[(&str, &str, &str, &str, &str)] = &[
    (
        "netflix",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-netflix.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/netflix.yaml",
        "classical",
    ),
    (
        "disney",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-disney.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/disney.yaml",
        "classical",
    ),
    (
        "openai",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-openai.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/openai.yaml",
        "classical",
    ),
    (
        "youtube",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-youtube.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/youtube.yaml",
        "classical",
    ),
    (
        "tiktok",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-tiktok.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/tiktok.yaml",
        "classical",
    ),
    (
        "hbo",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-hbo.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/hbo.yaml",
        "classical",
    ),
    (
        "primevideo",
        "https://raw.githubusercontent.com/SagerNet/sing-geosite/rule-set/geosite-primevideo.srs",
        "binary",
        "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/primevideo.yaml",
        "classical",
    ),
];

pub fn library_lookup(
    name: &str,
) -> Option<&'static (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
)> {
    RULE_SET_LIBRARY.iter().find(|e| e.0 == name)
}

/// System client provisioning for a relay rule.
/// Creates a client row and copies group links from the exit binding.
pub async fn provision_relay_client(
    db: &sea_orm::DatabaseConnection,
    rule_name: &str,
    exit_binding_id: Uuid,
) -> Result<Uuid, sea_orm::DbErr> {
    let client_id = Uuid::new_v4();
    let now = chrono::Utc::now().into();

    let active = client::ActiveModel {
        id: Set(client_id),
        user_id: Set(Uuid::new_v4()),
        name: Set(format!("relay-{}", rule_name)),
        email: Set(Some(format!(
            "relay-{}@relay.internal",
            &Uuid::new_v4().to_string()[..8]
        ))),
        traffic_limit_bytes: Set(0),
        traffic_used_bytes: Set(0),
        all_time_used_bytes: Set(0),
        expiry_date: Set(None),
        reset_day: Set(None),
        data_limit_reset_strategy: Set("no_reset".to_string()),
        last_traffic_reset_time: Set(None),
        max_devices: Set(None),
        status: Set("active".to_string()),
        on_hold_expire_duration_secs: Set(None),
        on_hold_timeout: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };
    active.insert(db).await?;

    // Copy group links from the exit binding
    let group_bindings = node_binding_group_binding::Entity::find()
        .filter(node_binding_group_binding::Column::NodeBindingId.eq(exit_binding_id))
        .all(db)
        .await?;

    for gb in group_bindings {
        let cgb = client_group_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            client_id: Set(client_id),
            group_id: Set(gb.group_id),
            created_at: Set(now),
        };
        cgb.insert(db).await?;
    }

    Ok(client_id)
}

async fn delete_relay_client(
    db: &sea_orm::DatabaseConnection,
    relay_client_id: Uuid,
) -> Result<(), sea_orm::DbErr> {
    // Delete client group bindings
    client_group_binding::Entity::delete_many()
        .filter(client_group_binding::Column::ClientId.eq(relay_client_id))
        .exec(db)
        .await?;

    // Delete the client itself
    client::Entity::delete_by_id(relay_client_id)
        .exec(db)
        .await?;

    Ok(())
}

fn relay_rule_to_json(r: relay_rule::Model) -> Value {
    json!({
        "id": r.id,
        "node_id": r.node_id,
        "exit_binding_id": r.exit_binding_id,
        "relay_client_id": r.relay_client_id,
        "name": r.name,
        "match_type": r.match_type,
        "match_config": r.match_config,
        "enabled": r.enabled,
        "sort_order": r.sort_order,
    })
}

async fn validate_relay_payload(
    db: &sea_orm::DatabaseConnection,
    node_id: Uuid,
    exit_binding_id: Uuid,
    match_type: &str,
    match_config: &Value,
) -> Result<(), ApiError> {
    // Node must exist
    let entry_node = node::Entity::find_by_id(node_id)
        .one(db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node not found"))?;
    let _ = entry_node; // unused but confirms existence

    // Exit binding must exist
    let exit_binding = node_binding::Entity::find_by_id(exit_binding_id)
        .one(db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("exit binding not found"))?;

    // No self-relay
    if exit_binding.node_id == node_id {
        return Err(ApiError::bad_request(
            "self_relay",
            "exit binding belongs to the same node as the entry node",
        ));
    }

    // Exit binding's protocol config must be a supported relay core+protocol
    let pc = protocol_config::Entity::find_by_id(exit_binding.protocol_config_id)
        .one(db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| {
            ApiError::bad_request(
                "protocol_config_missing",
                "exit binding's protocol config not found",
            )
        })?;

    if pc.core_type != "sing-box" && pc.core_type != "mihomo" {
        return Err(ApiError::bad_request(
            "unsupported_core",
            "exit binding core type must be sing-box or mihomo",
        ));
    }
    if pc.protocol_type != "vless_reality"
        && pc.protocol_type != "hysteria2"
        && pc.protocol_type != "anytls"
    {
        return Err(ApiError::bad_request(
            "unsupported_protocol",
            "exit binding protocol type must be vless_reality, hysteria2, or anytls",
        ));
    }

    // Validate match_type and match_config
    match match_type {
        "inline" => {
            let domains = match_config
                .get("domains")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let domain_suffixes = match_config
                .get("domain_suffixes")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if domains + domain_suffixes == 0 {
                return Err(ApiError::bad_request(
                    "invalid_match_config",
                    "inline match_config must contain at least one entry in 'domains' or 'domain_suffixes'",
                ));
            }
        }
        "rule_set" => {
            if let Some(library_name) = match_config.get("library").and_then(|v| v.as_str()) {
                if library_lookup(library_name).is_none() {
                    return Err(ApiError::bad_request(
                        "unknown_rule_set_library",
                        format!("unknown library rule set: {}", library_name),
                    ));
                }
            } else if let Some(custom) = match_config.get("custom") {
                let sb_url = custom
                    .get("singbox")
                    .and_then(|v| v.get("url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mh_url = custom
                    .get("mihomo")
                    .and_then(|v| v.get("url"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if sb_url.is_empty() || mh_url.is_empty() {
                    return Err(ApiError::bad_request(
                        "invalid_custom_rule_set",
                        "custom rule_set must have non-empty singbox.url and mihomo.url",
                    ));
                }
            } else {
                return Err(ApiError::bad_request(
                    "invalid_match_config",
                    "rule_set match_config must have 'library' or 'custom'",
                ));
            }
        }
        _ => {
            return Err(ApiError::bad_request(
                "invalid_match_type",
                "match_type must be 'inline' or 'rule_set'",
            ));
        }
    }

    Ok(())
}

pub async fn list_relay_rules(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let mut query = relay_rule::Entity::find();

    if let Some(node_id) = params.get("node_id") {
        if let Ok(id) = Uuid::parse_str(node_id) {
            query = query.filter(relay_rule::Column::NodeId.eq(id));
        }
    }

    let (rules, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            let total = query
                .clone()
                .count(&state.db)
                .await
                .map_err(ApiError::from)? as u64;
            let items = query
                .offset((page - 1) * per_page)
                .limit(per_page)
                .all(&state.db)
                .await
                .map_err(ApiError::from)?;
            (items, total)
        } else {
            let items = query.all(&state.db).await.map_err(ApiError::from)?;
            let total = items.len() as u64;
            (items, total)
        };

    let mut data = Vec::with_capacity(rules.len());
    for r in rules {
        let node_name = node::Entity::find_by_id(r.node_id)
            .one(&state.db)
            .await
            .map_err(ApiError::from)?
            .map(|n| n.name)
            .unwrap_or_default();

        let (exit_node_id, exit_node_name, exit_config_name) = if let Some(binding) =
            node_binding::Entity::find_by_id(r.exit_binding_id)
                .one(&state.db)
                .await
                .map_err(ApiError::from)?
        {
            let enode_name = node::Entity::find_by_id(binding.node_id)
                .one(&state.db)
                .await
                .map_err(ApiError::from)?
                .map(|n| n.name)
                .unwrap_or_default();
            let config_name = protocol_config::Entity::find_by_id(binding.protocol_config_id)
                .one(&state.db)
                .await
                .map_err(ApiError::from)?
                .map(|c| c.name)
                .unwrap_or_default();
            (binding.node_id, enode_name, config_name)
        } else {
            (Uuid::nil(), String::new(), String::new())
        };

        let mut obj = relay_rule_to_json(r);
        obj["node_name"] = json!(node_name);
        obj["exit_node_id"] = json!(exit_node_id);
        obj["exit_node_name"] = json!(exit_node_name);
        obj["exit_config_name"] = json!(exit_config_name);
        data.push(obj);
    }

    Ok(PaginatedResponse::new(data, total))
}

pub async fn library(State(state): State<Arc<AppState>>) -> ApiResult<Value> {
    let _ = state;
    let entries: Vec<Value> = RULE_SET_LIBRARY
        .iter()
        .map(|e| {
            json!({
                "name": e.0,
                "singbox_url": e.1,
                "singbox_format": e.2,
                "mihomo_url": e.3,
                "mihomo_behavior": e.4,
            })
        })
        .collect();
    Ok(ApiResponse::new(json!(entries)))
}

#[derive(serde::Deserialize)]
pub struct CreateRelayRulePayload {
    pub node_id: Uuid,
    pub exit_binding_id: Uuid,
    pub name: String,
    pub match_type: String,
    pub match_config: Value,
    pub enabled: Option<bool>,
    pub sort_order: Option<i32>,
}

pub async fn create_relay_rule(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateRelayRulePayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request("invalid_name", "name is required"));
    }

    validate_relay_payload(
        &state.db,
        payload.node_id,
        payload.exit_binding_id,
        &payload.match_type,
        &payload.match_config,
    )
    .await?;

    // Provision system client
    let relay_client_id = provision_relay_client(&state.db, &payload.name, payload.exit_binding_id)
        .await
        .map_err(ApiError::from)?;

    let now = chrono::Utc::now().into();
    let active = relay_rule::ActiveModel {
        id: Set(Uuid::new_v4()),
        node_id: Set(payload.node_id),
        exit_binding_id: Set(payload.exit_binding_id),
        relay_client_id: Set(relay_client_id),
        name: Set(payload.name.trim().to_string()),
        match_type: Set(payload.match_type),
        match_config: Set(payload.match_config),
        enabled: Set(payload.enabled.unwrap_or(true)),
        sort_order: Set(payload.sort_order.unwrap_or(0)),
        created_at: Set(now),
        updated_at: Set(now),
    };

    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    // Mark affected nodes as pending update
    let exit_node_id = node_binding::Entity::find_by_id(inserted.exit_binding_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .map(|b| b.node_id);

    let affected: Vec<Uuid> = [inserted.node_id, exit_node_id.unwrap_or(Uuid::nil())]
        .into_iter()
        .filter(|id| !id.is_nil())
        .collect();

    for core_type in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if let Err(e) = crate::service::protocol::mark_pending(
            &state.db,
            affected.clone(),
            core_type,
            crate::service::protocol::UPDATE_TYPE_CONFIG,
        )
        .await
        {
            tracing::warn!(
                "failed to mark pending for nodes after creating relay rule {}: {}",
                inserted.id,
                e
            );
        }
    }

    Ok(ApiResponse::new(relay_rule_to_json(inserted)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateRelayRulePayload {
    pub name: Option<String>,
    pub match_type: Option<String>,
    pub match_config: Option<Value>,
    pub enabled: Option<bool>,
    pub sort_order: Option<i32>,
    pub exit_binding_id: Option<Uuid>,
}

pub async fn update_relay_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRelayRulePayload>,
) -> ApiResult<Value> {
    let rule = relay_rule::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("relay rule not found"))?;

    let new_exit_binding_id = payload.exit_binding_id.unwrap_or(rule.exit_binding_id);
    let new_match_type = payload
        .match_type
        .clone()
        .unwrap_or_else(|| rule.match_type.clone());
    let new_match_config = payload
        .match_config
        .clone()
        .unwrap_or_else(|| rule.match_config.clone());

    validate_relay_payload(
        &state.db,
        rule.node_id,
        new_exit_binding_id,
        &new_match_type,
        &new_match_config,
    )
    .await?;

    // If exit_binding_id changed, reprovision the client
    let new_relay_client_id = if payload.exit_binding_id.is_some()
        && payload.exit_binding_id.unwrap() != rule.exit_binding_id
    {
        // Delete old client + group bindings
        delete_relay_client(&state.db, rule.relay_client_id)
            .await
            .map_err(ApiError::from)?;

        // Provision new client for the new exit binding
        let new_name = payload.name.as_deref().unwrap_or(&rule.name);
        provision_relay_client(&state.db, new_name, new_exit_binding_id)
            .await
            .map_err(ApiError::from)?
    } else {
        rule.relay_client_id
    };

    let mut active: relay_rule::ActiveModel = rule.clone().into();

    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "name cannot be empty",
            ));
        }
        active.name = Set(name.trim().to_string());
    }
    if payload.match_type.is_some() {
        active.match_type = Set(new_match_type);
    }
    if payload.match_config.is_some() {
        active.match_config = Set(new_match_config);
    }
    if let Some(enabled) = payload.enabled {
        active.enabled = Set(enabled);
    }
    if let Some(sort_order) = payload.sort_order {
        active.sort_order = Set(sort_order);
    }
    if payload.exit_binding_id.is_some() {
        active.exit_binding_id = Set(new_exit_binding_id);
        active.relay_client_id = Set(new_relay_client_id);
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    // Mark affected nodes as pending update
    let mut affected_nodes = vec![rule.node_id];

    let old_exit_node_id = node_binding::Entity::find_by_id(rule.exit_binding_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .map(|b| b.node_id);
    if let Some(nid) = old_exit_node_id {
        if nid != rule.node_id {
            affected_nodes.push(nid);
        }
    }

    let new_exit_node_id = node_binding::Entity::find_by_id(updated.exit_binding_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .map(|b| b.node_id);
    if let Some(nid) = new_exit_node_id {
        if nid != rule.node_id && !affected_nodes.contains(&nid) {
            affected_nodes.push(nid);
        }
    }

    for core_type in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if let Err(e) = crate::service::protocol::mark_pending(
            &state.db,
            affected_nodes.clone(),
            core_type,
            crate::service::protocol::UPDATE_TYPE_CONFIG,
        )
        .await
        {
            tracing::warn!(
                "failed to mark pending for nodes after updating relay rule {}: {}",
                updated.id,
                e
            );
        }
    }

    Ok(ApiResponse::new(relay_rule_to_json(updated)))
}

pub async fn delete_relay_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let rule = relay_rule::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("relay rule not found"))?;

    // Collect affected nodes before deleting
    let exit_node_id = node_binding::Entity::find_by_id(rule.exit_binding_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .map(|b| b.node_id);

    // Delete rule row
    relay_rule::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    // Delete relay client + group bindings
    delete_relay_client(&state.db, rule.relay_client_id)
        .await
        .map_err(ApiError::from)?;

    // Mark affected nodes as pending update
    let mut affected_nodes = vec![rule.node_id];
    if let Some(nid) = exit_node_id {
        if nid != rule.node_id {
            affected_nodes.push(nid);
        }
    }

    for core_type in [pp_common::CoreType::SingBox, pp_common::CoreType::Mihomo] {
        if let Err(e) = crate::service::protocol::mark_pending(
            &state.db,
            affected_nodes.clone(),
            core_type,
            crate::service::protocol::UPDATE_TYPE_CONFIG,
        )
        .await
        {
            tracing::warn!(
                "failed to mark pending for nodes after deleting relay rule {}: {}",
                id,
                e
            );
        }
    }

    Ok(axum::http::StatusCode::NO_CONTENT)
}
