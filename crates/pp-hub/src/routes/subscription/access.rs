use axum::{
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use pp_db::entities::{client, subscription};
use pp_subscription::{SubscriptionFormat, generate_subscription};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use std::collections::HashMap;
use std::sync::Arc;

use crate::response::ApiError;
use crate::state::AppState;

// ---------------------------------------------------------------------------
// Subscription Access Endpoint
// ---------------------------------------------------------------------------

/// Build the `subscription-userinfo` response header value (standard used by
/// Clash, sing-box, and V2Ray clients to display traffic/expiry info).
///
/// Format: `upload=xxx; download=xxx; total=xxx; expire=xxx`
fn format_userinfo_header(client: &client::Model) -> String {
    let upload = client.traffic_used_bytes;
    let total = client.traffic_limit_bytes;
    let expire = client.expiry_date.map(|e| e.timestamp()).unwrap_or(0);

    format!(
        "upload={}; download={}; total={}; expire={}",
        upload, upload, total, expire
    )
}

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
    let proxy_nodes = super::generator::build_proxy_nodes(&state.db, &client_model, filter_rules)
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

    let userinfo = format_userinfo_header(&client_model);
    let mut response = ([(header::CONTENT_TYPE, content_type)], content).into_response();
    if !userinfo.is_empty() {
        response.headers_mut().insert(
            header::HeaderName::from_static("subscription-userinfo"),
            header::HeaderValue::from_str(&userinfo)
                .unwrap_or_else(|_| header::HeaderValue::from_static("")),
        );
    }
    Ok(response)
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
) -> Result<Option<pp_db::entities::subscription_template::Model>, ApiError> {
    let format_lower = format.to_ascii_lowercase();

    if let Some(name) = template_name
        && let Some(t) = pp_db::entities::subscription_template::Entity::find()
            .filter(pp_db::entities::subscription_template::Column::Name.eq(name))
            .filter(pp_db::entities::subscription_template::Column::IsEnabled.eq(true))
            .one(db)
            .await
            .map_err(ApiError::from)?
    {
        return Ok(Some(t));
    }

    let templates = pp_db::entities::subscription_template::Entity::find()
        .filter(pp_db::entities::subscription_template::Column::Format.eq(&format_lower))
        .filter(pp_db::entities::subscription_template::Column::IsEnabled.eq(true))
        .all(db)
        .await
        .map_err(ApiError::from)?;

    Ok(templates.into_iter().next())
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

    let proxy_nodes = super::generator::build_proxy_nodes(&state.db, &client_model, None)
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
