//! Shared API response types and error conversions for Hub HTTP handlers.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

/// Standard API response envelope.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ApiResponse<T> {
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: serde::Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

/// Paginated API response envelope.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub total: u64,
}

impl<T> PaginatedResponse<T> {
    pub fn new(data: Vec<T>, total: u64) -> Self {
        Self { data, total }
    }
}

impl<T: serde::Serialize> IntoResponse for PaginatedResponse<T> {
    fn into_response(self) -> Response {
        Json(json!({
            "data": self.data,
            "meta": {
                "total": self.total,
            }
        }))
        .into_response()
    }
}

/// Structured API error.
#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_string(),
            message: message.into(),
        }
    }

    pub fn bad_request(code: &str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            message,
        )
    }

    #[allow(dead_code)]
    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", "authentication required")
    }

    #[allow(dead_code)]
    pub fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", "insufficient permissions")
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": {
                "code": self.code,
                "message": self.message,
            }
        }));
        (self.status, body).into_response()
    }
}

impl From<sea_orm::DbErr> for ApiError {
    fn from(err: sea_orm::DbErr) -> Self {
        tracing::error!("database error: {}", err);
        match err {
            sea_orm::DbErr::RecordNotInserted | sea_orm::DbErr::RecordNotUpdated => {
                Self::bad_request("db_conflict", "conflicting database operation")
            }
            _ => Self::internal("database operation failed"),
        }
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(_: serde_json::Error) -> Self {
        Self::bad_request("invalid_json", "invalid JSON")
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        tracing::error!("internal error: {}", err);
        Self::internal("internal server error")
    }
}

impl From<StatusCode> for ApiError {
    fn from(status: StatusCode) -> Self {
        Self::new(status, "http_error", status.canonical_reason().unwrap_or("error"))
    }
}

pub type ApiResult<T> = Result<ApiResponse<T>, ApiError>;
pub type PaginatedResult<T> = Result<PaginatedResponse<T>, ApiError>;

/// Convert the legacy `Result<Json<Value>, StatusCode>` into a consistent response.
#[allow(dead_code)]
pub fn json_response(data: Value) -> ApiResponse<Value> {
    ApiResponse { data }
}

/// Convert the legacy paginated builder into the new response type.
#[allow(dead_code)]
pub fn paginated_json_response(data: Vec<Value>, total: u64) -> PaginatedResponse<Value> {
    PaginatedResponse { data, total }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_error_serializes() {
        let err = ApiError::bad_request("bad_thing", "something went wrong");
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
