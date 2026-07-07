use axum::{
    extract::{FromRequestParts, Request, State},
    http::{StatusCode, request::Parts},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{DecodingKey, Header, Validation, decode, encode, errors::ErrorKind};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::state::AppState;

/// JWT claims for admin users.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub username: String,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
}

impl Claims {
    #[allow(dead_code)]
    pub fn has_role(&self, required: &str) -> bool {
        self.role == required || self.role == "admin"
    }
}

/// Represents an authenticated user extracted from JWT claims.
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub role: String,
}

impl<S: Send + Sync> FromRequestParts<S> for AuthUser {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<AuthUser>()
            .cloned()
            .ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// Middleware that validates `Authorization: Bearer <jwt>` header.
pub async fn require_auth(
    State(state): State<Arc<AppState>>,
    mut req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let secret = &state.config.jwt_secret;
    let decoding_key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();

    match decode::<Claims>(token, &decoding_key, &validation) {
        Ok(token_data) => {
            let claims = token_data.claims;
            let user_id =
                uuid::Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;

            let auth_user = AuthUser {
                user_id,
                username: claims.username,
                role: claims.role,
            };
            req.extensions_mut().insert(auth_user);
            Ok(next.run(req).await)
        }
        Err(e) => match e.kind() {
            ErrorKind::ExpiredSignature => Err(StatusCode::UNAUTHORIZED),
            _ => Err(StatusCode::UNAUTHORIZED),
        },
    }
}

/// Generate a JWT token for a given user.
pub fn create_jwt(
    user_id: uuid::Uuid,
    username: &str,
    role: &str,
    secret: &str,
    expiry_hours: u64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now();
    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        role: role.to_string(),
        iat: now.timestamp() as usize,
        exp: (now + chrono::Duration::hours(expiry_hours as i64)).timestamp() as usize,
    };

    let encoding_key = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());
    encode(&Header::default(), &claims, &encoding_key)
}
