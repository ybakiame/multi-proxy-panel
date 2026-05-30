use axum::{extract::Request, middleware::Next, response::Response};

/// Placeholder JWT auth middleware.
/// Will be expanded to validate `Authorization: Bearer <jwt>` header.
#[allow(dead_code)]
pub async fn require_auth(req: Request, next: Next) -> Response {
    // TODO: extract and validate JWT
    let _auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    // For now, allow all (development)
    next.run(req).await
}

/// Represents an authenticated user extracted from JWT claims.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuthUser {
    pub user_id: uuid::Uuid,
    pub username: String,
    pub role: String,
}
