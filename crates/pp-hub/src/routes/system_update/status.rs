use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::response::ApiError;

const STALE_AFTER_MINUTES: i64 = 30;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MaintenanceOutcome {
    Idle,
    Running,
    Succeeded,
    Failed,
    RolledBack,
    Stale,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct MaintenanceStatus {
    pub status: MaintenanceOutcome,
    pub stage: String,
    pub component: String,
    pub current_version: Option<String>,
    pub target_version: Option<String>,
    pub message: String,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

impl MaintenanceStatus {
    pub fn idle(current_version: &str) -> Self {
        Self {
            status: MaintenanceOutcome::Idle,
            stage: "idle".to_string(),
            component: "hub".to_string(),
            current_version: Some(current_version.to_string()),
            target_version: None,
            message: String::new(),
            started_at: None,
            finished_at: None,
        }
    }

    pub fn running(current_version: &str, target_version: &str) -> Self {
        Self {
            status: MaintenanceOutcome::Running,
            stage: "launching".to_string(),
            component: "hub".to_string(),
            current_version: Some(current_version.to_string()),
            target_version: Some(target_version.to_string()),
            message: "update submitted to systemd".to_string(),
            started_at: Some(Utc::now()),
            finished_at: None,
        }
    }

    pub fn failed(mut self, message: String) -> Self {
        self.status = MaintenanceOutcome::Failed;
        self.stage = "launching".to_string();
        self.message = message;
        self.finished_at = Some(Utc::now());
        self
    }

    pub fn is_in_progress(&self) -> bool {
        matches!(
            self.status,
            MaintenanceOutcome::Running | MaintenanceOutcome::Stale
        )
    }
}

pub(super) async fn read_status(
    path: &Path,
    current_version: &str,
) -> Result<MaintenanceStatus, ApiError> {
    let data = match tokio::fs::read(path).await {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(MaintenanceStatus::idle(current_version));
        }
        Err(error) => {
            return Err(ApiError::internal(format!(
                "failed to read maintenance status {}: {error}",
                path.display()
            )));
        }
    };
    let mut status: MaintenanceStatus = serde_json::from_slice(&data).map_err(|error| {
        ApiError::new(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "invalid_status_file",
            format!(
                "invalid maintenance status file {}: {error}",
                path.display()
            ),
        )
    })?;
    if status.status == MaintenanceOutcome::Running {
        let started_at = status.started_at.ok_or_else(|| {
            ApiError::new(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "invalid_status_file",
                format!(
                    "invalid maintenance status file {}: running status has no started_at",
                    path.display()
                ),
            )
        })?;
        if Utc::now().signed_duration_since(started_at) > Duration::minutes(STALE_AFTER_MINUTES) {
            status.status = MaintenanceOutcome::Stale;
            status.message =
                "running status exceeded 30 minutes; manual verification required".to_string();
        }
    }
    Ok(status)
}

pub(super) async fn write_status(path: &Path, status: &MaintenanceStatus) -> Result<(), ApiError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent).await.map_err(|error| {
            ApiError::internal(format!(
                "failed to create maintenance status directory {}: {error}",
                parent.display()
            ))
        })?;
    }
    let temporary = path.with_extension(format!("tmp-hub-{}", std::process::id()));
    let data = serde_json::to_vec_pretty(status).map_err(|error| {
        ApiError::internal(format!("failed to serialize maintenance status: {error}"))
    })?;
    let mut file = tokio::fs::File::create(&temporary).await.map_err(|error| {
        ApiError::internal(format!(
            "failed to create maintenance status {}: {error}",
            temporary.display()
        ))
    })?;
    file.write_all(&data).await.map_err(|error| {
        ApiError::internal(format!(
            "failed to write maintenance status {}: {error}",
            temporary.display()
        ))
    })?;
    file.write_all(b"\n").await.map_err(|error| {
        ApiError::internal(format!(
            "failed to write maintenance status {}: {error}",
            temporary.display()
        ))
    })?;
    file.sync_all().await.map_err(|error| {
        ApiError::internal(format!(
            "failed to sync maintenance status {}: {error}",
            temporary.display()
        ))
    })?;
    drop(file);
    tokio::fs::rename(&temporary, path).await.map_err(|error| {
        ApiError::internal(format!(
            "failed to replace maintenance status {}: {error}",
            path.display()
        ))
    })
}
