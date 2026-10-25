mod execution;
mod status;

use std::cmp::Ordering;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use pp_common::PanelError;
use pp_common::release::{GitHubReleaseClient, Release, compare_versions, parse_version};
use serde::{Deserialize, Serialize};

use crate::middleware::api_key::{scope_layer, scopes};
use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

use execution::spawn_update;
use status::{MaintenanceOutcome, MaintenanceStatus, read_status, write_status};

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/v1/system/update",
            get(get_update).route_layer(scope_layer(scopes::SYSTEM_READ)),
        )
        .route(
            "/api/v1/system/update",
            post(start_update).route_layer(scope_layer(scopes::SYSTEM_WRITE)),
        )
        .route(
            "/api/v1/system/update/status",
            get(get_update_status).route_layer(scope_layer(scopes::SYSTEM_READ)),
        )
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateRequest {
    target_version: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SystemUpdate {
    current_version: String,
    latest_version: String,
    update_available: bool,
    release_url: String,
    published_at: Option<DateTime<Utc>>,
    checked_at: DateTime<Utc>,
    execution_enabled: bool,
    manual_command: String,
    status: MaintenanceStatus,
}

pub async fn get_update(State(state): State<Arc<AppState>>) -> ApiResult<SystemUpdate> {
    let release = latest_release(&state).await?;
    let status = read_status(&state.config.maintenance_status_file, CURRENT_VERSION).await?;
    let update_available = compare_versions(CURRENT_VERSION, &release.tag)
        .map_err(release_validation_error)?
        == Ordering::Less;
    Ok(ApiResponse::new(SystemUpdate {
        current_version: CURRENT_VERSION.to_string(),
        latest_version: release.version.to_string(),
        update_available,
        release_url: release.html_url,
        published_at: release.published_at,
        checked_at: Utc::now(),
        execution_enabled: state.config.maintenance_enabled,
        manual_command: manual_command(&state),
        status,
    }))
}

pub async fn get_update_status(State(state): State<Arc<AppState>>) -> ApiResult<MaintenanceStatus> {
    Ok(ApiResponse::new(
        read_status(&state.config.maintenance_status_file, CURRENT_VERSION).await?,
    ))
}

pub async fn start_update(
    State(state): State<Arc<AppState>>,
    payload: Option<Json<UpdateRequest>>,
) -> Result<Response, ApiError> {
    if !state.config.maintenance_enabled {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "maintenance_disabled",
            "Hub-controlled maintenance is disabled",
        ));
    }
    ensure_not_running(&state).await?;
    let requested = payload.and_then(|Json(body)| body.target_version);
    let release = match requested.as_deref() {
        Some(version) => release_by_tag(&state, version).await?,
        None => latest_release(&state).await?,
    };
    validate_target(&release)?;
    if !state.maintenance.try_begin_update() {
        return Err(update_in_progress());
    }
    let result = execute_update(state.clone(), &release).await;
    if result.is_err() {
        state.maintenance.finish_update();
    }
    result
}

async fn execute_update(state: Arc<AppState>, release: &Release) -> Result<Response, ApiError> {
    ensure_file_not_running(&state).await?;
    let running = MaintenanceStatus::running(CURRENT_VERSION, &release.version.to_string());
    write_status(&state.config.maintenance_status_file, &running).await?;
    let mut child = match spawn_update(&state.config, release) {
        Ok(child) => child,
        Err(error) => {
            let failed = running
                .clone()
                .failed(format!("failed to spawn maintenance command: {error}"));
            if let Err(write_error) =
                write_status(&state.config.maintenance_status_file, &failed).await
            {
                tracing::error!("failed to record maintenance spawn failure: {write_error:?}");
            }
            return Err(ApiError::internal(format!(
                "failed to spawn maintenance command: {error}"
            )));
        }
    };

    let task_state = state.clone();
    let failed_status = running.clone();
    tokio::spawn(async move {
        let result = child.wait().await;
        if !matches!(result, Ok(exit) if exit.success()) {
            let message = match result {
                Ok(exit) => format!("maintenance command exited with {exit}"),
                Err(error) => format!("failed to wait for maintenance command: {error}"),
            };
            match read_status(&task_state.config.maintenance_status_file, CURRENT_VERSION).await {
                Ok(current) if current.status == MaintenanceOutcome::Running => {
                    if let Err(error) = write_status(
                        &task_state.config.maintenance_status_file,
                        &failed_status.failed(message),
                    )
                    .await
                    {
                        tracing::error!("failed to record maintenance command failure: {error:?}");
                    }
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::error!("failed to read maintenance command result: {error:?}");
                }
            }
        }
        task_state.maintenance.finish_update();
    });

    Ok((
        StatusCode::ACCEPTED,
        ApiResponse::new(serde_json::json!({
            "status": running,
            "message": "Hub update started",
        })),
    )
        .into_response())
}

async fn ensure_not_running(state: &Arc<AppState>) -> Result<(), ApiError> {
    if state.maintenance.update_in_progress() {
        return Err(update_in_progress());
    }
    ensure_file_not_running(state).await
}

async fn ensure_file_not_running(state: &Arc<AppState>) -> Result<(), ApiError> {
    let status = read_status(&state.config.maintenance_status_file, CURRENT_VERSION).await?;
    if status.is_in_progress() {
        return Err(update_in_progress());
    }
    Ok(())
}

fn update_in_progress() -> ApiError {
    ApiError::new(
        StatusCode::CONFLICT,
        "update_in_progress",
        "a Hub update is already running or requires manual verification",
    )
}

fn validate_target(release: &Release) -> Result<(), ApiError> {
    let tag_version = parse_version(&release.tag).map_err(release_validation_error)?;
    if tag_version != release.version || !release.version.pre.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_target_version",
            "target must be a stable release with a matching semantic-version tag",
        ));
    }
    if compare_versions(CURRENT_VERSION, &release.tag).map_err(release_validation_error)?
        != Ordering::Less
    {
        return Err(ApiError::bad_request(
            "no_update_available",
            "target version is not newer than the running Hub",
        ));
    }
    Ok(())
}

async fn latest_release(state: &Arc<AppState>) -> Result<Release, ApiError> {
    release_client(state)?
        .latest_stable()
        .await
        .map_err(release_error)
}

async fn release_by_tag(state: &Arc<AppState>, tag: &str) -> Result<Release, ApiError> {
    release_client(state)?
        .by_tag(tag)
        .await
        .map_err(release_error)
}

fn release_client(state: &Arc<AppState>) -> Result<GitHubReleaseClient, ApiError> {
    GitHubReleaseClient::new(&state.config.release_repo).map_err(|error| {
        ApiError::internal(format!(
            "invalid Hub release repository configuration: {error}"
        ))
    })
}

fn release_error(error: PanelError) -> ApiError {
    match error {
        PanelError::Validation(_) => release_validation_error(error),
        _ => ApiError::new(
            StatusCode::BAD_GATEWAY,
            "release_query_failed",
            error.to_string(),
        ),
    }
}

fn release_validation_error(error: PanelError) -> ApiError {
    ApiError::bad_request("invalid_target_version", error.to_string())
}

fn manual_command(state: &Arc<AppState>) -> String {
    format!(
        "sudo {} upgrade hub --version latest --repo {} --status-file {}",
        state.config.maintenance_cli_path.display(),
        state.config.release_repo,
        state.config.maintenance_status_file.display()
    )
}

#[cfg(test)]
mod tests;
