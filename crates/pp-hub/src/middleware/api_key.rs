use axum::{
    extract::{FromRequestParts, Request, State},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::Response,
};
use pp_db::entities::api_key;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use std::sync::Arc;

use crate::state::AppState;

/// Scope constants used for API key authorization.
pub mod scopes {
    pub const NODES_READ: &str = "nodes:read";
    pub const NODES_WRITE: &str = "nodes:write";
    pub const CLIENTS_READ: &str = "clients:read";
    pub const CLIENTS_WRITE: &str = "clients:write";
    pub const PROTOCOLS_READ: &str = "protocols:read";
    pub const PROTOCOLS_WRITE: &str = "protocols:write";
    pub const BINDINGS_READ: &str = "bindings:read";
    pub const BINDINGS_WRITE: &str = "bindings:write";
    pub const GROUPS_READ: &str = "groups:read";
    pub const GROUPS_WRITE: &str = "groups:write";
    pub const SUBSCRIPTIONS_READ: &str = "subscriptions:read";
    pub const SUBSCRIPTIONS_WRITE: &str = "subscriptions:write";
    pub const TEMPLATES_READ: &str = "templates:read";
    pub const TEMPLATES_WRITE: &str = "templates:write";
    pub const API_KEYS_READ: &str = "apikeys:read";
    pub const API_KEYS_WRITE: &str = "apikeys:write";
    pub const WEBHOOKS_READ: &str = "webhooks:read";
    pub const WEBHOOKS_WRITE: &str = "webhooks:write";
    pub const TRAFFIC_READ: &str = "traffic:read";
    pub const METRICS_READ: &str = "metrics:read";
    pub const ONLINES_READ: &str = "onlines:read";
    pub const LOGS_READ: &str = "logs:read";
    pub const ALL: &str = "*";

    pub const ALL_SCOPES: &[&str] = &[
        NODES_READ,
        NODES_WRITE,
        CLIENTS_READ,
        CLIENTS_WRITE,
        PROTOCOLS_READ,
        PROTOCOLS_WRITE,
        BINDINGS_READ,
        BINDINGS_WRITE,
        GROUPS_READ,
        GROUPS_WRITE,
        SUBSCRIPTIONS_READ,
        SUBSCRIPTIONS_WRITE,
        TEMPLATES_READ,
        TEMPLATES_WRITE,
        API_KEYS_READ,
        API_KEYS_WRITE,
        WEBHOOKS_READ,
        WEBHOOKS_WRITE,
        TRAFFIC_READ,
        METRICS_READ,
        ONLINES_READ,
        LOGS_READ,
        ALL,
    ];
}

/// API Key authentication result attached to request extensions.
#[derive(Clone, Debug)]
pub struct ApiKeyAuth {
    #[allow(dead_code)]
    pub key_id: uuid::Uuid,
    #[allow(dead_code)]
    pub name: String,
    pub scopes: Vec<String>,
}

impl ApiKeyAuth {
    pub fn has_scope(&self, required: &str) -> bool {
        self.scopes
            .iter()
            .any(|s| s == required || s == scopes::ALL)
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ApiKeyAuth {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<ApiKeyAuth>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
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

    // Check in-memory cache first to avoid expensive Argon2 verification on every request.
    let cache_key = crate::state::ApiKeyCache::compute_key(&key_str);
    if let Some(cached) = state.api_key_cache.get(&cache_key) {
        // Re-validate expiration since it may have changed.
        if let Some(expires) = cached.expires_at {
            if expires < chrono::Utc::now() {
                state.api_key_cache.invalidate();
                return Err(StatusCode::UNAUTHORIZED);
            }
        }

        // Re-validate IP allowlist since it may have changed.
        if let Some(ref allowlist) = cached.ip_allowlist {
            let client_ip = extract_client_ip(&req, state.config.trusted_proxy_ips.as_ref());

            if let Some(ips) = allowlist.as_array() {
                let allowed: Vec<String> = ips
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect();
                if !allowed.is_empty() && !allowed.iter().any(|ip| ip_matches(&client_ip, ip)) {
                    return Err(StatusCode::FORBIDDEN);
                }
            }
        }

        // Rate limit check
        if let Some(limit) = cached.rate_limit {
            if limit > 0 {
                let key = format!("rate:apikey:{}", cached.key_id);
                let allowed = state.rate_limiter.check(&key, limit as u64).await;
                if !allowed {
                    return Err(StatusCode::TOO_MANY_REQUESTS);
                }
            }
        }

        let auth = ApiKeyAuth {
            key_id: cached.key_id,
            name: cached.name,
            scopes: cached.scopes,
        };

        req.extensions_mut().insert(auth);
        return Ok(next.run(req).await);
    }

    let key_record = api_key::Entity::find()
        .filter(api_key::Column::IsActive.eq(true))
        .all(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let key_record = key_record
        .into_iter()
        .find(|k| pp_common::verify_secret(&key_str, &k.key_hash).unwrap_or(false))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Check expiration
    if let Some(expires) = key_record.expires_at {
        if expires < chrono::Utc::now() {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    // Check IP allowlist
    if let Some(allowlist) = key_record.ip_allowlist.clone() {
        let client_ip = extract_client_ip(&req, state.config.trusted_proxy_ips.as_ref());

        if let Some(ips) = allowlist.as_array() {
            let allowed: Vec<String> = ips
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !allowed.is_empty() && !allowed.iter().any(|ip| ip_matches(&client_ip, ip)) {
                return Err(StatusCode::FORBIDDEN);
            }
        }
    }

    // Rate limit check
    if let Some(limit) = key_record.rate_limit {
        if limit > 0 {
            let key = format!("rate:apikey:{}", key_record.id);
            let allowed = state.rate_limiter.check(&key, limit as u64).await;
            if !allowed {
                return Err(StatusCode::TOO_MANY_REQUESTS);
            }
        }
    }

    let scopes: Vec<String> = key_record
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
        name: key_record.name.clone(),
        scopes: scopes.clone(),
    };

    // Cache successful verification.
    state.api_key_cache.insert(
        cache_key,
        crate::state::CachedApiKey {
            key_id: key_record.id,
            name: key_record.name,
            scopes,
            expires_at: key_record.expires_at,
            ip_allowlist: key_record.ip_allowlist,
            rate_limit: key_record.rate_limit,
            cached_at: std::time::Instant::now(),
        },
    );

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

fn extract_client_ip(
    req: &Request,
    trusted_proxies: Option<&std::collections::HashSet<std::net::IpAddr>>,
) -> String {
    let direct_ip = req
        .extensions()
        .get::<std::net::SocketAddr>()
        .map(|addr| addr.ip());

    let forwarded = req
        .headers()
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.split(',').next())
        .and_then(|s| s.trim().parse::<std::net::IpAddr>().ok());

    match (direct_ip, forwarded, trusted_proxies) {
        (_, Some(forwarded_ip), Some(trusted)) => {
            if direct_ip.map(|ip| trusted.contains(&ip)).unwrap_or(false) {
                forwarded_ip.to_string()
            } else {
                direct_ip.map(|ip| ip.to_string()).unwrap_or_default()
            }
        }
        (Some(ip), _, _) => ip.to_string(),
        _ => String::new(),
    }
}

fn ip_matches(client_ip: &str, pattern: &str) -> bool {
    if pattern.contains('/') {
        if let Ok(net) = pattern.parse::<ipnet::IpNet>() {
            if let Ok(ip) = client_ip.parse::<std::net::IpAddr>() {
                return net.contains(&ip);
            }
        }
        false
    } else {
        client_ip == pattern
    }
}

type ScopeFuture =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<Response, StatusCode>> + Send>>;

/// Middleware function that checks if the authenticated API key has the required scope.
pub async fn require_scope_middleware(
    scope: &'static str,
    auth: ApiKeyAuth,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    if auth.has_scope(scope) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Build a Tower layer that enforces the given scope.
pub fn scope_layer(
    scope: &'static str,
) -> axum::middleware::FromFnLayer<
    impl Clone + Fn(ApiKeyAuth, Request, Next) -> ScopeFuture,
    (),
    (ApiKeyAuth, Request),
> {
    #[derive(Clone)]
    struct ScopeState(&'static str);

    let scope_fn = move |auth: ApiKeyAuth, req: Request, next: Next| {
        let state = ScopeState(scope);
        Box::pin(async move { require_scope_middleware(state.0, auth, req, next).await })
            as ScopeFuture
    };

    axum::middleware::from_fn(scope_fn)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ip_matching() {
        assert!(ip_matches("192.168.1.1", "192.168.1.1"));
        assert!(!ip_matches("192.168.1.100", "192.168.1.1"));
        assert!(ip_matches("192.168.1.100", "192.168.1.0/24"));
        assert!(!ip_matches("10.0.0.1", "192.168.1.0/24"));
    }

    #[test]
    fn scope_check() {
        let auth = ApiKeyAuth {
            key_id: uuid::Uuid::new_v4(),
            name: "test".into(),
            scopes: vec!["nodes:read".into()],
        };
        assert!(auth.has_scope("nodes:read"));
        assert!(!auth.has_scope("nodes:write"));

        let wildcard = ApiKeyAuth {
            key_id: uuid::Uuid::new_v4(),
            name: "admin".into(),
            scopes: vec!["*".into()],
        };
        assert!(wildcard.has_scope("anything:write"));
    }
}
