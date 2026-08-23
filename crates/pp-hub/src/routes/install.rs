//! Install script and one-click install command endpoints.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
};
use pp_db::entities::node;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

/// Serve an install bootstrap script with the release repo placeholder replaced.
fn render_script(
    raw: &str,
    release_repo: &str,
) -> Result<(axum::http::StatusCode, HeaderMap, String), ApiError> {
    let script = raw.replace("__PROXYPANEL_RELEASE_REPO__", release_repo);

    let mut headers = HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "text/x-shellscript; charset=utf-8"
            .parse()
            .map_err(|_| ApiError::internal("invalid content-type header"))?,
    );
    headers.insert(
        axum::http::header::CACHE_CONTROL,
        "no-store"
            .parse()
            .map_err(|_| ApiError::internal("invalid cache-control header"))?,
    );

    Ok((axum::http::StatusCode::OK, headers, script))
}

/// Serve the agent install shell script.
/// The script is embedded at compile time and the release repo placeholder is replaced.
pub async fn serve_install_script(
    State(state): State<Arc<AppState>>,
) -> Result<(axum::http::StatusCode, HeaderMap, String), ApiError> {
    let script = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/install-agent.sh"
    ));
    render_script(script, &state.config.release_repo)
}

/// Serve the hub install bootstrap script (same placeholder replacement).
pub async fn serve_hub_install_script(
    State(state): State<Arc<AppState>>,
) -> Result<(axum::http::StatusCode, HeaderMap, String), ApiError> {
    let script = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/install-hub.sh"
    ));
    render_script(script, &state.config.release_repo)
}

/// Generate a one-click install command for a specific node.
/// Rotates the node's token and returns a curl | bash command.
pub async fn node_install_command(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> ApiResult<Value> {
    let n = node::Entity::find_by_id(id)
        .one(&state.db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("node not found"))?;

    // Check whether the node has ever connected before moving `n` into ActiveModel
    let was_connected = n.last_seen_at.is_some();

    // Rotate token
    let raw_token = pp_common::generate_secure_token();
    let token_hash = pp_common::hash_secret_async(raw_token.clone())
        .await
        .map_err(|e| ApiError::internal(format!("failed to hash token: {e}")))?;

    let mut active: node::ActiveModel = n.clone().into();
    active.token_hash = Set(token_hash);
    active.updated_at = Set(chrono::Utc::now().into());
    active.update(&state.db).await.map_err(ApiError::from)?;

    // Determine script URL
    let script_url = if let Some(ref public_url) = state.config.public_http_url {
        let base = public_url.trim_end_matches('/');
        format!("{}/install.sh", base)
    } else {
        let host = headers
            .get(axum::http::header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost:8081");
        let scheme = headers
            .get("x-forwarded-proto")
            .and_then(|h| h.to_str().ok())
            .unwrap_or_else(|| {
                if state.config.http_tls_cert.is_some() {
                    "https"
                } else {
                    "http"
                }
            });
        format!("{}://{}/install.sh", scheme, host)
    };

    // Determine hub (gRPC) URL
    let hub_url = if let Some(ref public_grpc) = state.config.public_grpc_url {
        public_grpc.clone()
    } else {
        let host = headers
            .get(axum::http::header::HOST)
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost:8081");
        let host_without_port = host.split(':').next().unwrap_or(host);
        let grpc_port = state
            .config
            .grpc_listen
            .split(':')
            .next_back()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(50052);
        let scheme = headers
            .get("x-forwarded-proto")
            .and_then(|h| h.to_str().ok())
            .unwrap_or_else(|| {
                if state.config.http_tls_cert.is_some() {
                    "https"
                } else {
                    "http"
                }
            });
        format!("{}://{}:{}", scheme, host_without_port, grpc_port)
    };

    let version = concat!("v", env!("CARGO_PKG_VERSION"));

    let command = format!(
        "curl -fsSL '{}' | bash -s -- --hub-url '{}' --token '{}' --agent-id '{}' --name '{}' --version '{}'",
        shell_quote(&script_url),
        shell_quote(&hub_url),
        shell_quote(&raw_token),
        shell_quote(&id.to_string()),
        shell_quote(&n.name),
        shell_quote(version),
    );

    Ok(ApiResponse::new(json!({
        "id": id,
        "name": n.name,
        "token": raw_token,
        "hub_url": hub_url,
        "script_url": script_url,
        "version": version,
        "command": command,
        "was_connected": was_connected,
    })))
}

/// Escape a string for safe use inside single-quoted shell arguments.
fn shell_quote(s: &str) -> String {
    s.replace('\'', "'\\''")
}
