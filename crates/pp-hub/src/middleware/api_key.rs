use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
    http::StatusCode,
};
use pp_db::entities::api_key;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;

use crate::state::AppState;

/// API Key authentication result attached to request extensions.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct ApiKeyAuth {
    pub key_id: uuid::Uuid,
    pub name: String,
    pub scopes: Vec<String>,
}

impl ApiKeyAuth {
    #[allow(dead_code)]
    pub fn has_scope(&self, required: &str) -> bool {
        self.scopes.iter().any(|s| s == required || s == "*")
    }
}

/// Middleware that validates `X-API-Key` or `Authorization: Bearer <key>` header.
pub async fn require_api_key(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let key_str = extract_key_from_request(&req);

    if key_str.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let key_hash = sha256_truncated(&key_str);

    let key_record = api_key::Entity::find()
        .filter(api_key::Column::KeyHash.eq(&key_hash))
        .filter(api_key::Column::IsActive.eq(true))
        .one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check expiration
    if let Some(expires) = key_record.expires_at {
        if expires < chrono::Utc::now() {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // Check IP allowlist
    if let Some(allowlist) = key_record.ip_allowlist {
        let client_ip = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|h| h.to_str().ok())
            .map(|s| s.to_string())
            .or_else(|| {
                req.extensions()
                    .get::<std::net::SocketAddr>()
                    .map(|addr| addr.ip().to_string())
            })
            .unwrap_or_default();

        if let Some(ips) = allowlist.as_array() {
            let allowed: Vec<String> = ips
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !allowed.is_empty() && !allowed.iter().any(|ip| client_ip.starts_with(ip)) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    let scopes = key_record
        .scopes
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let auth = ApiKeyAuth {
        key_id: key_record.id,
        name: key_record.name,
        scopes,
    };

    req.extensions_mut().insert(auth);
    Ok(next.run(req).await)
}

fn extract_key_from_request(req: &Request) -> String {
    // Try X-API-Key header first
    if let Some(header) = req.headers().get("x-api-key") {
        if let Ok(val) = header.to_str() {
            return val.trim().to_string();
        }
    }
    // Fallback to Authorization: Bearer <key>
    if let Some(header) = req.headers().get("authorization") {
        if let Ok(val) = header.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return token.trim().to_string();
            }
        }
    }
    String::new()
}

fn sha256_truncated(input: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..16])
}

/// Middleware that checks if the authenticated API key has the required scope.
#[allow(dead_code)]
pub fn require_scope(scope: &'static str) -> impl Fn(ApiKeyAuth) -> Result<ApiKeyAuth, StatusCode> {
    move |auth: ApiKeyAuth| {
        if auth.has_scope(scope) {
            Ok(auth)
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}
