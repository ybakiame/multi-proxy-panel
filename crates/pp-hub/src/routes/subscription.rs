use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use pp_db::entities::{
    client, client_group_binding, node, node_binding, node_group, node_group_binding,
    protocol_config, subscription, subscription_template,
};
use pp_subscription::{ProxyNode, SubscriptionFormat, generate_subscription};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult, PaginatedResponse, PaginatedResult};
use crate::state::AppState;

// ========== Subscription Templates ==========

fn template_to_json(t: subscription_template::Model) -> Value {
    json!({
        "id": t.id,
        "name": t.name,
        "format": t.format,
        "base_config": t.base_config,
        "filter_rules": t.filter_rules,
        "custom_headers": t.custom_headers,
        "created_at": t.created_at,
        "updated_at": t.updated_at,
    })
}

pub async fn list_templates(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (templates, total) = if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
        let total = subscription_template::Entity::find()
            .count(&state.db)
            .await
            .map_err(ApiError::from)? as u64;
        let items = subscription_template::Entity::find()
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        (items, total)
    } else {
        let items = subscription_template::Entity::find()
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        let total = items.len() as u64;
        (items, total)
    };

    let data: Vec<Value> = templates.into_iter().map(template_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateTemplatePayload {
    pub name: String,
    pub format: Option<String>,
    pub base_config: Option<Value>,
    pub filter_rules: Option<Value>,
    pub custom_headers: Option<Value>,
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTemplatePayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request("invalid_name", "template name is required"));
    }

    let active = subscription_template::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(payload.name),
        format: Set(payload.format.unwrap_or_else(|| "base64".to_string())),
        base_config: Set(payload.base_config),
        filter_rules: Set(payload.filter_rules),
        custom_headers: Set(payload.custom_headers),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active
        .insert(&state.db)
        .await
        .map_err(ApiError::from)?;
    Ok(ApiResponse::new(template_to_json(inserted)))
}

pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = subscription_template::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;
    if res.rows_affected == 0 {
        Err(ApiError::not_found("subscription template not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

// ========== Subscriptions ==========

fn subscription_to_json(s: subscription::Model) -> Value {
    json!({
        "id": s.id,
        "client_id": s.client_id,
        "template_id": s.template_id,
        "token": s.token,
        "url_path": s.url_path,
        "is_active": s.is_active,
        "expire_at": s.expire_at.map(|d| d.to_rfc3339()),
        "last_accessed_at": s.last_accessed_at.map(|d| d.to_rfc3339()),
        "created_at": s.created_at,
    })
}

pub async fn list_subscriptions(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let (subs, total) = if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
        let total = subscription::Entity::find()
            .count(&state.db)
            .await
            .map_err(ApiError::from)? as u64;
        let items = subscription::Entity::find()
            .offset((page - 1) * per_page)
            .limit(per_page)
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        (items, total)
    } else {
        let items = subscription::Entity::find()
            .all(&state.db)
            .await
            .map_err(ApiError::from)?;
        let total = items.len() as u64;
        (items, total)
    };

    let data: Vec<Value> = subs.into_iter().map(subscription_to_json).collect();
    Ok(PaginatedResponse::new(data, total))
}

#[derive(serde::Deserialize)]
pub struct CreateSubscriptionPayload {
    pub client_id: Uuid,
    pub template_id: Uuid,
}

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateSubscriptionPayload>,
) -> ApiResult<Value> {
    let token = pp_common::generate_secure_token();
    let url_path = format!("/sub/{}", token);

    let active = subscription::ActiveModel {
        id: Set(Uuid::new_v4()),
        client_id: Set(payload.client_id),
        template_id: Set(payload.template_id),
        token: Set(token.clone()),
        url_path: Set(url_path.clone()),
        expire_at: Set(None),
        is_active: Set(true),
        last_accessed_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active
        .insert(&state.db)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiResponse::new(json!({
        "id": inserted.id,
        "token": token,
        "url_path": url_path,
    })))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateSubscriptionPayload {
    pub is_active: Option<bool>,
    pub expire_at: Option<String>,
}

pub async fn update_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSubscriptionPayload>,
) -> ApiResult<Value> {
    let sub = subscription::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription not found"))?;

    let mut active: subscription::ActiveModel = sub.into();

    if let Some(is_active) = payload.is_active {
        active.is_active = Set(is_active);
    }
    if payload.expire_at.is_some() {
        let expire_at = payload
            .expire_at
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok());
        active.expire_at = Set(expire_at);
    }

    let updated = active
        .update(&state.db)
        .await
        .map_err(ApiError::from)?;

    Ok(ApiResponse::new(subscription_to_json(updated)))
}

pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = subscription::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;
    if res.rows_affected == 0 {
        Err(ApiError::not_found("subscription not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

// ========== Subscription Access Endpoint ==========

pub async fn serve_subscription(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    // Find subscription by token
    let sub = subscription::Entity::find()
        .filter(subscription::Column::Token.eq(&token))
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription not found"))?;

    if !sub.is_active {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "subscription_inactive", "subscription is inactive"));
    }

    // Update last_accessed_at
    let mut active: subscription::ActiveModel = sub.clone().into();
    active.last_accessed_at = Set(Some(chrono::Utc::now().into()));
    let _ = active.update(&state.db).await;

    // Get template
    let template = subscription_template::Entity::find_by_id(sub.template_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription template not found"))?;

    // Determine format
    let format_param = params
        .get("format")
        .map(|s| s.as_str())
        .unwrap_or(&template.format);
    let format = format_param
        .parse::<SubscriptionFormat>()
        .map_err(|_| ApiError::bad_request("invalid_format", "unknown subscription format"))?;

    // Get client for credential injection
    let client_model = client::Entity::find_by_id(sub.client_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("client not found"))?;

    // Build proxy nodes from all active bindings + configs
    let filter_rules = template.filter_rules.as_ref();
    let proxy_nodes = build_proxy_nodes(&state.db, &client_model, filter_rules)
        .await
        .map_err(ApiError::from)?;

    let base_config = template.base_config.as_ref();
    let content = generate_subscription(format, &proxy_nodes, base_config).map_err(|e| {
        tracing::warn!("subscription generation error: {}", e);
        ApiError::internal(format!("subscription generation failed: {e}"))
    })?;

    let content_type = match format {
        SubscriptionFormat::Json | SubscriptionFormat::SingBox | SubscriptionFormat::V2RayNG => {
            "application/json"
        }
        SubscriptionFormat::Clash => "application/x-yaml",
        SubscriptionFormat::Base64 => "text/plain; charset=utf-8",
    };

    Ok(([(header::CONTENT_TYPE, content_type)], content).into_response())
}

/// Fetch group IDs assigned to a client.
async fn get_client_group_ids(
    db: &sea_orm::DatabaseConnection,
    client_id: Uuid,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let bindings = client_group_binding::Entity::find()
        .filter(client_group_binding::Column::ClientId.eq(client_id))
        .all(db)
        .await?;
    Ok(bindings.into_iter().map(|b| b.group_id).collect())
}

/// Fetch group IDs assigned to a node.
async fn get_node_group_ids(
    db: &sea_orm::DatabaseConnection,
    node_id: Uuid,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let bindings = node_group_binding::Entity::find()
        .filter(node_group_binding::Column::NodeId.eq(node_id))
        .all(db)
        .await?;
    Ok(bindings.into_iter().map(|b| b.group_id).collect())
}

async fn build_proxy_nodes(
    db: &sea_orm::DatabaseConnection,
    client: &client::Model,
    filter_rules: Option<&Value>,
) -> Result<Vec<ProxyNode>, sea_orm::DbErr> {
    // Skip clients that are limited or expired
    if client.status == "limited" || client.status == "expired" {
        return Ok(Vec::new());
    }

    let client_group_ids = get_client_group_ids(db, client.id).await?;
    let client_has_groups = !client_group_ids.is_empty();

    // Parse filter rules
    let allowed_protocols = filter_rules
        .and_then(|r| r.get("protocols"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();

    let allowed_node_group_names = filter_rules
        .and_then(|r| r.get("node_groups"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<std::collections::HashSet<_>>()
        })
        .unwrap_or_default();

    // Resolve group names to IDs for filtering
    let allowed_node_group_ids: std::collections::HashSet<Uuid> = if !allowed_node_group_names.is_empty() {
        let all_groups = node_group::Entity::find()
            .all(db)
            .await?;
        all_groups
            .into_iter()
            .filter(|g| allowed_node_group_names.contains(&g.name))
            .map(|g| g.id)
            .collect()
    } else {
        std::collections::HashSet::new()
    };

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
            let protocol = parse_protocol_type(&cfg.protocol_type);
            if protocol.is_err() {
                continue;
            }

            let protocol_type = protocol.unwrap();

            // Apply filter: protocol type
            if !allowed_protocols.is_empty()
                && !allowed_protocols.contains(&protocol_type.to_string())
            {
                continue;
            }

            // Group-based access control
            let node_group_ids = get_node_group_ids(db, node.id).await?;
            let node_has_groups = !node_group_ids.is_empty();

            if node_has_groups && client_has_groups {
                // Both have groups — check intersection
                let has_overlap = node_group_ids.iter().any(|g| client_group_ids.contains(g));
                if !has_overlap {
                    continue; // Skip: client has no access to this node
                }
            }

            // Apply filter: node groups (by group name)
            if !allowed_node_group_ids.is_empty() {
                let has_matching_group = node_group_ids
                    .iter()
                    .any(|gid| allowed_node_group_ids.contains(gid));
                if !has_matching_group {
                    continue;
                }
            }

            // Inject client credentials into settings
            let mut settings = cfg.settings.clone();
            inject_client_credentials(&mut settings, client, &cfg.protocol_type);

            nodes.push(ProxyNode {
                name: format!("{}-{}", node.name, cfg.name),
                protocol: protocol_type,
                server: node.address.clone(),
                port: cfg.listen_port as u16,
                settings,
                tls: cfg.tls_settings.clone(),
            });
        }
    }

    Ok(nodes)
}

fn inject_client_credentials(settings: &mut Value, client: &client::Model, protocol_type: &str) {
    if let Some(obj) = settings.as_object_mut() {
        // Ensure clients array exists
        if !obj.contains_key("clients") {
            obj.insert("clients".to_string(), json!([]));
        }

        match protocol_type {
            // UUID-based protocols (VLESS, VMess, Trojan)
            pt if pt.starts_with("vless") || pt == "vmess" || pt == "trojan" => {
                let flow = if pt == "vless_vision" || pt == "vless_reality" {
                    "xtls-rprx-vision"
                } else {
                    ""
                };
                let mut client_obj = json!({
                    "id": client.id.to_string(),
                    "email": client.email.as_ref().unwrap_or(&client.name),
                    "flow": flow,
                });
                if let Some(limit) = client.max_devices {
                    if limit > 0 {
                        client_obj.as_object_mut().unwrap().insert("limitIp".to_string(), json!(limit));
                    }
                }
                // Also expose the current client's id at top level for link generators.
                obj.insert("id".to_string(), json!(client.id.to_string()));
                if let Some(arr) = obj.get_mut("clients").and_then(|v| v.as_array_mut()) {
                    arr.push(client_obj);
                }
            }
            // Password-based protocols
            "hysteria2" | "anytls" => {
                let client_obj = json!({
                    "name": client.email.as_ref().unwrap_or(&client.name),
                    "password": client.id.to_string(),
                });
                if let Some(arr) = obj.get_mut("clients").and_then(|v| v.as_array_mut()) {
                    arr.push(client_obj);
                }
            }
            // TUIC uses UUID + password
            "tuic" | "tuic_v5" => {
                let client_obj = json!({
                    "name": client.email.as_ref().unwrap_or(&client.name),
                    "uuid": client.id.to_string(),
                    "password": client.id.to_string().replace("-", ""),
                });
                if let Some(arr) = obj.get_mut("clients").and_then(|v| v.as_array_mut()) {
                    arr.push(client_obj);
                }
            }
            // Shadowsocks
            "shadowsocks2022" => {
                obj.insert(
                    "password".to_string(),
                    json!(client.id.to_string().replace("-", "")),
                );
            }
            _ => {}
        }
    }
}

fn parse_protocol_type(s: &str) -> Result<pp_common::ProtocolType, ()> {
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
        _ => Err(()),
    }
}
