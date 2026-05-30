use axum::{
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use pp_db::entities::{client, node, node_binding, protocol_config, subscription, subscription_template};
use pp_subscription::{generate_subscription, ProxyNode, SubscriptionFormat};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::state::AppState;

// ========== Subscription Templates ==========

pub async fn list_templates(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let templates = subscription_template::Entity::find().all(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let data: Vec<Value> = templates.into_iter().map(|t| json!({
        "id": t.id, "name": t.name, "format": t.format,
        "base_config": t.base_config, "filter_rules": t.filter_rules,
    })).collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let name = payload.get("name").and_then(|v| v.as_str()).ok_or(StatusCode::BAD_REQUEST)?;
    let active = subscription_template::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        format: Set(payload.get("format").and_then(|v| v.as_str()).unwrap_or("base64").to_string()),
        base_config: Set(payload.get("base_config").cloned()),
        filter_rules: Set(payload.get("filter_rules").cloned()),
        custom_headers: Set(payload.get("custom_headers").cloned()),
        created_at: Set(chrono::Utc::now().into()),
        updated_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "data": { "id": inserted.id, "name": inserted.name } })))
}

pub async fn delete_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = subscription_template::Entity::delete_by_id(id).exec(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected == 0 { Err(StatusCode::NOT_FOUND) } else { Ok(StatusCode::NO_CONTENT) }
}

// ========== Subscriptions ==========

pub async fn list_subscriptions(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let subs = subscription::Entity::find().all(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let data: Vec<Value> = subs.into_iter().map(|s| json!({
        "id": s.id, "client_id": s.client_id, "template_id": s.template_id,
        "token": s.token, "url_path": s.url_path, "is_active": s.is_active,
    })).collect();
    Ok(Json(json!({ "data": data })))
}

pub async fn create_subscription(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    let client_id = payload.get("client_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok()).ok_or(StatusCode::BAD_REQUEST)?;
    let template_id = payload.get("template_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok()).ok_or(StatusCode::BAD_REQUEST)?;

    let token = pp_common::generate_secure_token();
    let url_path = format!("/sub/{}", token);

    let active = subscription::ActiveModel {
        id: Set(Uuid::new_v4()),
        client_id: Set(client_id),
        template_id: Set(template_id),
        token: Set(token.clone()),
        url_path: Set(url_path.clone()),
        expire_at: Set(None),
        is_active: Set(true),
        last_accessed_at: Set(None),
        created_at: Set(chrono::Utc::now().into()),
    };
    let inserted = active.insert(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({ "data": {
        "id": inserted.id,
        "token": token,
        "url_path": url_path,
    } })))
}

pub async fn delete_subscription(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let res = subscription::Entity::delete_by_id(id).exec(&state.db).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if res.rows_affected == 0 { Err(StatusCode::NOT_FOUND) } else { Ok(StatusCode::NO_CONTENT) }
}

// ========== Subscription Access Endpoint ==========

pub async fn serve_subscription(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Response, StatusCode> {
    // Find subscription by token
    let sub = subscription::Entity::find()
        .filter(subscription::Column::Token.eq(&token))
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if !sub.is_active {
        return Err(StatusCode::FORBIDDEN);
    }

    // Update last_accessed_at
    let mut active: subscription::ActiveModel = sub.clone().into();
    active.last_accessed_at = Set(Some(chrono::Utc::now().into()));
    let _ = active.update(&state.db).await;

    // Get template
    let template = subscription_template::Entity::find_by_id(sub.template_id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Determine format
    let format_param = params.get("format").map(|s| s.as_str()).unwrap_or(&template.format);
    let format = format_param.parse::<SubscriptionFormat>().map_err(|_| StatusCode::BAD_REQUEST)?;

    // Get client for credential injection
    let client_model = client::Entity::find_by_id(sub.client_id)
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Build proxy nodes from all active bindings + configs
    let proxy_nodes = build_proxy_nodes(&state.db, &client_model).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let base_config = template.base_config.as_ref();
    let content = generate_subscription(format, &proxy_nodes, base_config)
        .map_err(|e| {
            tracing::warn!("subscription generation error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let content_type = match format {
        SubscriptionFormat::Json | SubscriptionFormat::SingBox | SubscriptionFormat::V2RayNG => "application/json",
        SubscriptionFormat::Clash => "application/x-yaml",
        SubscriptionFormat::Base64 => "text/plain; charset=utf-8",
    };

    Ok((
        [(header::CONTENT_TYPE, content_type)],
        content,
    ).into_response())
}

async fn build_proxy_nodes(
    db: &sea_orm::DatabaseConnection,
    client: &client::Model,
) -> Result<Vec<ProxyNode>, sea_orm::DbErr> {
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

            // Inject client credentials into settings
            let mut settings = cfg.settings.clone();
            inject_client_credentials(&mut settings, client, &cfg.protocol_type);

            nodes.push(ProxyNode {
                name: format!("{}-{}", node.name, cfg.name),
                protocol: protocol.unwrap(),
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
                let client_obj = json!({
                    "id": client.id.to_string(),
                    "email": client.email.as_ref().unwrap_or(&client.name),
                    "flow": flow,
                });
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
                obj.insert("password".to_string(), json!(client.id.to_string().replace("-", "")));
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
