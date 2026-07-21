use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json, Response},
};
use pp_db::entities::{
    certificate, client, client_group_binding, inbound_host, node, node_binding,
    node_binding_group_binding, node_group, protocol_config, subscription, subscription_template,
};
use pp_subscription::{ProxyNode, SubscriptionFormat, generate_subscription};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect, Set,
};
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
        "is_builtin": t.is_builtin,
        "is_enabled": t.is_enabled,
        "created_at": t.created_at,
        "updated_at": t.updated_at,
    })
}

pub async fn list_templates(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> PaginatedResult<Value> {
    let items = subscription_template::Entity::find()
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let total = items.len() as u64;
    let templates: Vec<Value> =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
            items
                .into_iter()
                .skip(((page - 1) * per_page) as usize)
                .take(per_page as usize)
                .map(template_to_json)
                .collect()
        } else {
            items.into_iter().map(template_to_json).collect()
        };

    Ok(PaginatedResponse::new(templates, total))
}

#[derive(serde::Deserialize)]
pub struct CreateTemplatePayload {
    pub name: String,
    pub format: Option<String>,
    pub base_config: Option<String>,
    pub filter_rules: Option<Value>,
    pub custom_headers: Option<Value>,
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTemplatePayload>,
) -> ApiResult<Value> {
    if payload.name.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_name",
            "template name is required",
        ));
    }

    let format = payload.format.unwrap_or_else(|| "base64".to_string());
    let active = subscription_template::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        name: Set(payload.name),
        format: Set(format.clone()),
        base_config: Set(payload.base_config),
        filter_rules: Set(payload.filter_rules),
        custom_headers: Set(payload.custom_headers),
        is_builtin: Set(false),
        is_enabled: Set(true),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

    // Enforce only one enabled template per format.
    enforce_unique_enabled_template(&state.db, inserted.id.clone(), &format, true).await?;

    Ok(ApiResponse::new(template_to_json(inserted)))
}

#[derive(serde::Deserialize, Default)]
pub struct UpdateTemplatePayload {
    pub name: Option<String>,
    pub format: Option<String>,
    pub base_config: Option<String>,
    pub filter_rules: Option<Value>,
    pub custom_headers: Option<Value>,
    pub is_enabled: Option<bool>,
}

pub async fn update_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTemplatePayload>,
) -> ApiResult<Value> {
    let template = subscription_template::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription template not found"))?;

    // Builtin templates may only have their enabled flag changed.
    if template.is_builtin
        && (payload.name.is_some()
            || payload.format.is_some()
            || payload.base_config.is_some()
            || payload.filter_rules.is_some()
            || payload.custom_headers.is_some())
    {
        return Err(ApiError::bad_request(
            "builtin_readonly",
            "builtin templates can only be enabled or disabled",
        ));
    }

    let mut active: subscription_template::ActiveModel = template.clone().into();
    if let Some(name) = payload.name {
        if name.trim().is_empty() {
            return Err(ApiError::bad_request(
                "invalid_name",
                "template name is required",
            ));
        }
        active.name = Set(name);
    }
    let mut new_format = template.format.clone();
    if let Some(format) = payload.format {
        active.format = Set(format.clone());
        new_format = format;
    }
    if payload.base_config.is_some() {
        active.base_config = Set(payload.base_config);
    }
    if payload.filter_rules.is_some() {
        active.filter_rules = Set(payload.filter_rules);
    }
    if payload.custom_headers.is_some() {
        active.custom_headers = Set(payload.custom_headers);
    }
    let mut new_enabled = template.is_enabled;
    if let Some(enabled) = payload.is_enabled {
        active.is_enabled = Set(enabled);
        new_enabled = enabled;
    }
    active.updated_at = Set(chrono::Utc::now().into());

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

    // Enforce only one enabled template per format.
    enforce_unique_enabled_template(&state.db, updated.id.clone(), &new_format, new_enabled)
        .await?;

    Ok(ApiResponse::new(template_to_json(updated)))
}

pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<axum::http::StatusCode, ApiError> {
    let template = subscription_template::Entity::find_by_id(id.clone())
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription template not found"))?;

    if template.is_builtin {
        return Err(ApiError::bad_request(
            "builtin_protected",
            "builtin templates cannot be deleted",
        ));
    }

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
    let (subs, total) =
        if let Some((page, per_page)) = crate::routes::common::parse_pagination(&params) {
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
        token: Set(token.clone()),
        url_path: Set(url_path.clone()),
        expire_at: Set(None),
        is_active: Set(true),
        last_accessed_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(ApiError::from)?;

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

    let updated = active.update(&state.db).await.map_err(ApiError::from)?;

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
    headers: axum::http::HeaderMap,
) -> Result<Response, ApiError> {
    // Find subscription by token
    let sub = subscription::Entity::find()
        .filter(subscription::Column::Token.eq(&token))
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription not found"))?;

    if !sub.is_active {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "subscription_inactive",
            "subscription is inactive",
        ));
    }

    // Update last_accessed_at
    let mut active: subscription::ActiveModel = sub.clone().into();
    active.last_accessed_at = Set(Some(chrono::Utc::now().into()));
    let _ = active.update(&state.db).await;

    // Determine format from query param or User-Agent
    let format = detect_subscription_format(&params, headers.get(axum::http::header::USER_AGENT));

    // Find template matching the requested format
    let template_opt = find_template_for_format(
        &state.db,
        format.as_str(),
        params.get("template").map(|s| s.as_str()),
    )
    .await?;

    let mut target_format = format
        .parse::<SubscriptionFormat>()
        .map_err(|_| ApiError::bad_request("invalid_format", "unknown subscription format"))?;

    // Fall back to base64 when no enabled template exists for the requested format.
    let template = match template_opt {
        Some(t) => Some(t),
        None if target_format != SubscriptionFormat::Base64 => {
            target_format = SubscriptionFormat::Base64;
            find_template_for_format(
                &state.db,
                "base64",
                params.get("template").map(|s| s.as_str()),
            )
            .await?
        }
        None => None,
    };

    // Get client for credential injection
    let client_model = client::Entity::find_by_id(sub.client_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("client not found"))?;

    // Build proxy nodes from all active bindings + configs
    let filter_rules = template.as_ref().and_then(|t| t.filter_rules.as_ref());
    let proxy_nodes = build_proxy_nodes(&state.db, &client_model, filter_rules)
        .await
        .map_err(ApiError::from)?;

    let base_config = template.as_ref().and_then(|t| t.base_config.as_deref());
    let content = generate_subscription(target_format, &proxy_nodes, base_config).map_err(|e| {
        tracing::warn!("subscription generation error: {}", e);
        ApiError::internal(format!("subscription generation failed: {e}"))
    })?;

    let content_type = match target_format {
        SubscriptionFormat::Json | SubscriptionFormat::SingBox | SubscriptionFormat::V2RayNG => {
            "application/json"
        }
        SubscriptionFormat::Clash => "application/x-yaml",
        SubscriptionFormat::Base64 => "text/plain; charset=utf-8",
    };

    Ok(([(header::CONTENT_TYPE, content_type)], content).into_response())
}

/// Detect subscription format from query parameter or User-Agent string.
fn detect_subscription_format(
    params: &HashMap<String, String>,
    user_agent: Option<&axum::http::HeaderValue>,
) -> String {
    if let Some(format) = params.get("format") {
        return format.to_ascii_lowercase();
    }

    let ua = user_agent
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ua.contains("clash") || ua.contains("mihomo") {
        "clash".to_string()
    } else if ua.contains("sing-box") || ua.contains("singbox") {
        "sing-box".to_string()
    } else if ua.contains("v2rayng") || ua.contains("v2ray") {
        "v2rayng".to_string()
    } else {
        "base64".to_string()
    }
}

/// Find an enabled subscription template matching the requested format.
/// If `template_name` is provided, try to find a template with that name
/// and fallback to the first enabled template of the requested format.
/// Returns None if no matching enabled template exists; callers should fall
/// back to the base64 format which does not require a template.
async fn find_template_for_format(
    db: &sea_orm::DatabaseConnection,
    format: &str,
    template_name: Option<&str>,
) -> Result<Option<subscription_template::Model>, ApiError> {
    let format_lower = format.to_ascii_lowercase();

    if let Some(name) = template_name {
        if let Some(t) = subscription_template::Entity::find()
            .filter(subscription_template::Column::Name.eq(name))
            .filter(subscription_template::Column::IsEnabled.eq(true))
            .one(db)
            .await
            .map_err(ApiError::from)?
        {
            return Ok(Some(t));
        }
    }

    let templates = subscription_template::Entity::find()
        .filter(subscription_template::Column::Format.eq(&format_lower))
        .filter(subscription_template::Column::IsEnabled.eq(true))
        .all(db)
        .await
        .map_err(ApiError::from)?;

    Ok(templates.into_iter().next())
}

/// Ensure at most one template per format is enabled.
/// When `enabled` is true, disable all other enabled templates of the same format.
async fn enforce_unique_enabled_template(
    db: &sea_orm::DatabaseConnection,
    template_id: String,
    format: &str,
    enabled: bool,
) -> Result<(), ApiError> {
    if !enabled {
        return Ok(());
    }

    let others = subscription_template::Entity::find()
        .filter(subscription_template::Column::Id.ne(template_id))
        .filter(subscription_template::Column::Format.eq(format))
        .filter(subscription_template::Column::IsEnabled.eq(true))
        .all(db)
        .await
        .map_err(ApiError::from)?;

    for other in others {
        let mut active: subscription_template::ActiveModel = other.into();
        active.is_enabled = Set(false);
        active.updated_at = Set(chrono::Utc::now().into());
        active.update(db).await.map_err(ApiError::from)?;
    }

    Ok(())
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

/// Fetch group IDs assigned to a node binding.
async fn get_binding_group_ids(
    db: &sea_orm::DatabaseConnection,
    node_binding_id: Uuid,
) -> Result<Vec<Uuid>, sea_orm::DbErr> {
    let bindings = node_binding_group_binding::Entity::find()
        .filter(node_binding_group_binding::Column::NodeBindingId.eq(node_binding_id))
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
    let allowed_node_group_ids: std::collections::HashSet<Uuid> =
        if !allowed_node_group_names.is_empty() {
            let all_groups = node_group::Entity::find().all(db).await?;
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
            let protocol_type = match parse_protocol_type(&cfg.protocol_type) {
                Ok(pt) => pt,
                Err(_) => continue,
            };

            // Apply filter: protocol type
            if !allowed_protocols.is_empty()
                && !allowed_protocols.contains(&protocol_type.to_string())
            {
                continue;
            }

            // Group-based access control.
            // A client must belong to at least one group. It can only access bindings
            // that share at least one group with it. No groups for the client means
            // no binding access.
            let binding_group_ids = get_binding_group_ids(db, binding.id).await?;
            let binding_has_groups = !binding_group_ids.is_empty();

            if client_group_ids.is_empty() {
                continue;
            }

            if !binding_has_groups
                || !binding_group_ids
                    .iter()
                    .any(|g| client_group_ids.contains(g))
            {
                continue;
            }

            // Apply template filter: node groups (by group name)
            if !allowed_node_group_ids.is_empty() {
                let has_matching_group = binding_group_ids
                    .iter()
                    .any(|gid| allowed_node_group_ids.contains(gid));
                if !has_matching_group {
                    continue;
                }
            }

            // Inject client credentials into settings
            let mut settings = cfg.settings.clone();
            inject_client_credentials(&mut settings, client, &cfg.protocol_type);

            // Determine effective address/port: check for inbound_host override
            let host_override = inbound_host::Entity::find()
                .filter(inbound_host::Column::ProtocolConfigId.eq(cfg.id))
                .filter(inbound_host::Column::NodeId.eq(node.id))
                .filter(inbound_host::Column::IsActive.eq(true))
                .one(db)
                .await?;

            let (effective_server, effective_port) = if let Some(ref host) = host_override {
                (host.address.clone(), host.port as u16)
            } else if let Some(parent_id) = node.parent_id {
                // Relay/child node: use parent node's domain/address with this node's port
                let parent = node::Entity::find_by_id(parent_id).one(db).await?;
                if let Some(parent) = parent {
                    (
                        parent
                            .domain
                            .clone()
                            .unwrap_or_else(|| parent.address.clone()),
                        cfg.listen_port as u16,
                    )
                } else {
                    (
                        node.domain.clone().unwrap_or_else(|| node.address.clone()),
                        cfg.listen_port as u16,
                    )
                }
            } else {
                (
                    node.domain.clone().unwrap_or_else(|| node.address.clone()),
                    cfg.listen_port as u16,
                )
            };

            // Apply host overrides to settings (sni, host, path)
            if let Some(ref host) = host_override {
                if let Some(obj) = settings.as_object_mut() {
                    if let Some(sni) = &host.sni {
                        obj.insert("sni".to_string(), json!(sni));
                    }
                    if let Some(host_val) = &host.host {
                        obj.insert("host".to_string(), json!(host_val));
                    }
                    if let Some(path) = &host.path {
                        obj.insert("path".to_string(), json!(path));
                    }
                }
                if let Some(tls) = cfg.tls_settings.as_ref() {
                    if let Some(tls_obj) = tls.as_object() {
                        let mut new_tls = tls_obj.clone();
                        if let Some(sni) = &host.sni {
                            new_tls.insert("serverName".to_string(), json!(sni));
                        }
                        if let Some(alpn) = &host.alpn {
                            new_tls.insert(
                                "alpn".to_string(),
                                json!(alpn.split(',').collect::<Vec<_>>()),
                            );
                        }
                        if let Some(fp) = &host.fingerprint {
                            new_tls.insert("fingerprint".to_string(), json!(fp));
                        }
                        // Re-assign to settings.tls via a separate variable
                    }
                }
            }

            // Build node name with variable substitution
            let display_name = if let Some(ref host) = host_override {
                substitute_variables(&host.remark, client, &cfg, &node)
            } else {
                format!("{}-{}", node.name, cfg.name)
            };

            let tls = pp_common::settings_helper::merge_tls_settings(
                cfg.tls_settings.clone(),
                binding
                    .override_settings
                    .as_ref()
                    .and_then(|o| o.get("tls_settings").cloned()),
            );
            let tls = resolve_subscription_tls(db, binding.node_id, tls).await?;

            nodes.push(ProxyNode {
                name: display_name,
                protocol: protocol_type,
                server: effective_server,
                port: effective_port,
                settings,
                tls,
            });
        }
    }

    Ok(nodes)
}

/// Normalize TLS settings for client links: managed certificates contribute
/// their domain as the SNI (serverName), and an ACME domain doubles as the
/// SNI when nothing more specific is set.
async fn resolve_subscription_tls(
    db: &sea_orm::DatabaseConnection,
    node_id: Uuid,
    tls: Option<Value>,
) -> Result<Option<Value>, sea_orm::DbErr> {
    let Some(mut tls) = tls else {
        return Ok(None);
    };

    let cert_id = tls
        .get("cert_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok());
    if let Some(cert_id) = cert_id {
        if let Some(cert) = certificate::Entity::find_by_id(cert_id).one(db).await? {
            if cert.node_id == node_id {
                if let Some(obj) = tls.as_object_mut() {
                    obj.insert("serverName".to_string(), json!(cert.domain));
                    obj.remove("cert_id");
                }
            }
        }
    }

    let has_server_name = tls
        .get("serverName")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());
    let domain = tls
        .get("domain")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if !has_server_name {
        if let Some(domain) = domain {
            if let Some(obj) = tls.as_object_mut() {
                obj.insert("serverName".to_string(), json!(domain));
            }
        }
    }

    Ok(Some(tls))
}

/// Substitute template variables in a string.
/// Supported: {USERNAME}, {DATA_USED}, {DATA_LEFT}, {DATA_LIMIT}, {DAYS_LEFT}, {EXPIRE_DATE}, {STATUS}, {PROTOCOL}, {TRANSPORT}
fn substitute_variables(
    template: &str,
    client: &client::Model,
    cfg: &protocol_config::Model,
    node: &node::Model,
) -> String {
    let data_used = client.traffic_used_bytes;
    let data_limit = client.traffic_limit_bytes;
    let data_left = if data_limit > 0 {
        (data_limit - data_used).max(0)
    } else {
        0
    };
    let days_left = client
        .expiry_date
        .map(|e| {
            let diff = e.signed_duration_since(chrono::Utc::now().fixed_offset());
            diff.num_days().max(0)
        })
        .unwrap_or(0);

    let expire_date_str = client
        .expiry_date
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "never".to_string());

    let transport_str = cfg
        .settings
        .get("network")
        .and_then(|v| v.as_str())
        .unwrap_or("tcp")
        .to_string();

    template
        .replace("{USERNAME}", &client.name)
        .replace("{DATA_USED}", &format_bytes(data_used))
        .replace("{DATA_LEFT}", &format_bytes(data_left))
        .replace("{DATA_LIMIT}", &format_bytes(data_limit))
        .replace("{DAYS_LEFT}", &days_left.to_string())
        .replace("{EXPIRE_DATE}", &expire_date_str)
        .replace("{STATUS}", &client.status)
        .replace("{PROTOCOL}", &cfg.protocol_type)
        .replace("{TRANSPORT}", &transport_str)
        .replace(
            "{SERVER_IP}",
            &node.domain.clone().unwrap_or_else(|| node.address.clone()),
        )
}

/// Format bytes as human-readable string (e.g. "1.5 GB").
fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    const TB: i64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn inject_client_credentials(settings: &mut Value, client: &client::Model, protocol_type: &str) {
    if let Some(obj) = settings.as_object_mut() {
        // Ensure clients array exists
        if !obj.contains_key("clients") {
            obj.insert("clients".to_string(), json!([]));
        }

        match protocol_type {
            // UUID-based VLESS protocols
            pt if pt.starts_with("vless") => {
                let flow = if pt == "vless_reality" {
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
                        if let Some(obj) = client_obj.as_object_mut() {
                            obj.insert("limitIp".to_string(), json!(limit));
                        }
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
            _ => {}
        }
    }
}

fn parse_protocol_type(s: &str) -> Result<pp_common::ProtocolType, ()> {
    use pp_common::ProtocolType;
    match s {
        "vless_reality" => Ok(ProtocolType::VlessReality),
        "vless_xhttp" => Ok(ProtocolType::VlessXhttp),
        "hysteria2" => Ok(ProtocolType::Hysteria2),
        "anytls" => Ok(ProtocolType::Anytls),
        _ => Err(()),
    }
}

/// Generate a subscription link for a subscription token and optional format.
async fn build_subscription_link(
    state: &Arc<AppState>,
    token: &str,
    format: &str,
) -> Result<String, ApiError> {
    let sub = subscription::Entity::find()
        .filter(subscription::Column::Token.eq(token))
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription not found"))?;

    let client_model = client::Entity::find_by_id(sub.client_id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("client not found"))?;

    let proxy_nodes = build_proxy_nodes(&state.db, &client_model, None)
        .await
        .map_err(ApiError::from)?;

    let fmt = format
        .parse::<SubscriptionFormat>()
        .map_err(|_| ApiError::bad_request("invalid_format", "unknown subscription format"))?;

    let content = generate_subscription(fmt, &proxy_nodes, None).map_err(|e| {
        tracing::warn!("subscription generation error: {}", e);
        ApiError::internal(format!("subscription generation failed: {e}"))
    })?;

    Ok(content)
}

/// GET /sub/{token}/qr — Return an SVG QR code of the default subscription link.
pub async fn serve_subscription_qr(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, ApiError> {
    // Validate subscription exists and is active
    let sub = subscription::Entity::find()
        .filter(subscription::Column::Token.eq(&token))
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("subscription not found"))?;

    if !sub.is_active {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "subscription_inactive",
            "subscription is inactive",
        ));
    }

    // Update last_accessed_at
    let mut active: subscription::ActiveModel = sub.clone().into();
    active.last_accessed_at = Set(Some(chrono::Utc::now().into()));
    let _ = active.update(&state.db).await;

    let format = params.get("format").map(|s| s.as_str()).unwrap_or("base64");

    let content = build_subscription_link(&state, &token, format).await?;

    // Generate SVG QR code
    let code = qrcode::QrCode::new(content.as_bytes())
        .map_err(|e| ApiError::internal(format!("failed to generate QR code: {e}")))?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(200, 200)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();

    Ok(([(header::CONTENT_TYPE, "image/svg+xml")], svg).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_db::entities::{client, node, protocol_config};

    #[test]
    fn parse_protocol_type_works() {
        assert!(parse_protocol_type("vless_reality").is_ok());
        assert!(parse_protocol_type("hysteria2").is_ok());
        assert!(parse_protocol_type("unknown").is_err());
    }

    #[test]
    fn format_bytes_works() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(1024_i64 * 1024 * 1024 * 1024), "1.0 TB");
    }

    fn make_test_client() -> client::Model {
        client::Model {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            name: "testuser".to_string(),
            email: Some("test@example.com".to_string()),
            traffic_limit_bytes: 10 * 1024 * 1024 * 1024, // 10 GB
            traffic_used_bytes: 2 * 1024 * 1024 * 1024,   // 2 GB used
            all_time_used_bytes: 5 * 1024 * 1024 * 1024,
            expiry_date: Some((chrono::Utc::now() + chrono::Duration::days(30)).into()),
            reset_day: None,
            data_limit_reset_strategy: "monthly".to_string(),
            last_traffic_reset_time: None,
            max_devices: Some(3),
            status: "active".to_string(),
            on_hold_expire_duration_secs: None,
            on_hold_timeout: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn make_test_config() -> protocol_config::Model {
        protocol_config::Model {
            id: Uuid::new_v4(),
            name: "vless-reality".to_string(),
            protocol_type: "vless_reality".to_string(),
            core_type: "xray".to_string(),
            core_version: None,
            listen_port: 443,
            listen_address: "0.0.0.0".to_string(),
            settings: serde_json::json!({"network": "tcp"}),
            tls_settings: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    fn make_test_node() -> node::Model {
        node::Model {
            id: Uuid::new_v4(),
            name: "us-node".to_string(),
            hostname: "us-server".to_string(),
            address: "1.2.3.4".to_string(),
            domain: None,
            token_hash: "hash".to_string(),
            cores_available: serde_json::json!(["xray"]),
            labels: None,
            usage_coefficient: 1.0,
            status: "online".to_string(),
            parent_id: None,
            last_seen_at: None,
            created_at: chrono::Utc::now().into(),
            updated_at: chrono::Utc::now().into(),
        }
    }

    #[test]
    fn substitute_variables_replaces_all_placeholders() {
        let client = make_test_client();
        let config = make_test_config();
        let node = make_test_node();

        let template = "{USERNAME}-{PROTOCOL}-{TRANSPORT}-{SERVER_IP}-{STATUS}-{DAYS_LEFT}-{DATA_USED}-{DATA_LEFT}-{DATA_LIMIT}-{EXPIRE_DATE}";
        let result = substitute_variables(template, &client, &config, &node);

        assert!(result.contains("testuser"));
        assert!(result.contains("vless_reality"));
        assert!(result.contains("tcp"));
        assert!(result.contains("1.2.3.4"));
        assert!(result.contains("active"));
        assert!(result.contains("2.0 GB")); // DATA_USED
        assert!(result.contains("8.0 GB")); // DATA_LEFT (10-2)
        assert!(result.contains("10.0 GB")); // DATA_LIMIT
    }

    #[test]
    fn substitute_variables_handles_no_expiry() {
        let mut client = make_test_client();
        client.expiry_date = None;
        let config = make_test_config();
        let node = make_test_node();

        let result = substitute_variables("{EXPIRE_DATE}-{DAYS_LEFT}", &client, &config, &node);
        assert!(result.contains("never"));
        assert!(result.contains("0"));
    }
}
