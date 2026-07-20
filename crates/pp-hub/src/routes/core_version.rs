//! Core version catalog: user-curated upstream release/prerelease versions.
//!
//! The catalog is populated by the user selecting versions fetched from
//! GitHub releases (`/upstream` is read-only, `/` accepts the selection).
//! Protocol configs reference saved versions via `core_version`, and the
//! agent installs the matching binary on demand.

use axum::{
    Json,
    extract::{Path, Query, State},
};
use pp_common::CoreType;
use pp_db::entities::core_version;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

use crate::response::{ApiError, ApiResponse, ApiResult};
use crate::state::AppState;

const GITHUB_CONNECT_TIMEOUT_SECS: u64 = 10;
const GITHUB_REQUEST_TIMEOUT_SECS: u64 = 60;
const VERSIONS_PER_CHANNEL: usize = 10;

const CHANNEL_RELEASE: &str = "release";
const CHANNEL_PRERELEASE: &str = "prerelease";

const ALL_CORES: [CoreType; 3] = [CoreType::Xray, CoreType::SingBox, CoreType::Mihomo];

fn version_to_json(v: &core_version::Model) -> Value {
    json!({
        "id": v.id,
        "core_type": v.core_type,
        "version": v.version,
        "channel": v.channel,
        "created_at": v.created_at,
    })
}

pub async fn list_core_versions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Value> {
    let mut query = core_version::Entity::find();
    if let Some(core) = params.get("core_type").filter(|s| !s.is_empty()) {
        query = query.filter(core_version::Column::CoreType.eq(core));
    }
    let items = query
        .order_by_asc(core_version::Column::CoreType)
        .order_by_desc(core_version::Column::Channel)
        .order_by_desc(core_version::Column::CreatedAt)
        .all(&state.db)
        .await
        .map_err(ApiError::from)?;

    let data: Vec<Value> = items.iter().map(version_to_json).collect();
    Ok(ApiResponse::new(json!({ "versions": data })))
}

pub async fn delete_core_version(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<axum::http::StatusCode, ApiError> {
    let res = core_version::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(ApiError::from)?;

    if res.rows_affected == 0 {
        Err(ApiError::not_found("core version not found"))
    } else {
        Ok(axum::http::StatusCode::NO_CONTENT)
    }
}

struct UpstreamRelease {
    tag: String,
    channel: &'static str,
}

async fn fetch_upstream_versions(core_type: CoreType) -> Result<Vec<UpstreamRelease>, ApiError> {
    let (owner, repo) = core_type.github_repo();
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(GITHUB_CONNECT_TIMEOUT_SECS))
        .timeout(std::time::Duration::from_secs(GITHUB_REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| ApiError::internal(format!("failed to build http client: {}", e)))?;

    let url = format!(
        "https://api.github.com/repos/{}/{}/releases?per_page=30",
        owner, repo
    );
    let resp = client
        .get(&url)
        .header("User-Agent", "proxy-panel-hub")
        .send()
        .await
        .map_err(|e| ApiError::internal(format!("GitHub API request failed: {}", e)))?;
    if !resp.status().is_success() {
        return Err(ApiError::internal(format!(
            "GitHub API returned status {} for {}/{}",
            resp.status(),
            owner,
            repo
        )));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| ApiError::internal(format!("failed to read GitHub releases body: {}", e)))?;
    let releases: Vec<Value> = serde_json::from_str(&body).map_err(|e| {
        ApiError::internal(format!(
            "failed to parse GitHub releases for {}/{}: {}",
            owner, repo, e
        ))
    })?;

    let mut stable: Vec<UpstreamRelease> = Vec::new();
    let mut pre: Vec<UpstreamRelease> = Vec::new();
    for release in releases {
        let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) else {
            continue;
        };
        let is_pre = release
            .get("prerelease")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_pre {
            if pre.len() < VERSIONS_PER_CHANNEL {
                pre.push(UpstreamRelease {
                    tag: tag.to_string(),
                    channel: CHANNEL_PRERELEASE,
                });
            }
        } else if stable.len() < VERSIONS_PER_CHANNEL {
            stable.push(UpstreamRelease {
                tag: tag.to_string(),
                channel: CHANNEL_RELEASE,
            });
        }
        if stable.len() >= VERSIONS_PER_CHANNEL && pre.len() >= VERSIONS_PER_CHANNEL {
            break;
        }
    }

    stable.extend(pre);
    Ok(stable)
}

/// Read-only view of upstream GitHub releases, annotated with whether each
/// version is already saved in the catalog.
pub async fn list_upstream_versions(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Value> {
    let cores: Vec<CoreType> = match params.get("core_type").filter(|s| !s.is_empty()) {
        Some(raw) => vec![
            raw.parse::<CoreType>()
                .map_err(|_| ApiError::bad_request("invalid_core_type", "unknown core type"))?,
        ],
        None => ALL_CORES.to_vec(),
    };

    let saved: std::collections::HashSet<(String, String)> = core_version::Entity::find()
        .all(&state.db)
        .await
        .map_err(ApiError::from)?
        .into_iter()
        .map(|v| (v.core_type, v.version))
        .collect();

    let mut data = Vec::new();
    for core_type in cores {
        let upstream = fetch_upstream_versions(core_type).await?;
        let versions: Vec<Value> = upstream
            .into_iter()
            .map(|r| {
                json!({
                    "version": r.tag,
                    "channel": r.channel,
                    "saved": saved.contains(&(core_type.to_string(), r.tag.clone())),
                })
            })
            .collect();
        data.push(json!({
            "core_type": core_type.to_string(),
            "versions": versions,
        }));
    }

    Ok(ApiResponse::new(json!({ "cores": data })))
}

#[derive(serde::Deserialize)]
pub struct SaveVersionsPayload {
    pub versions: Vec<SaveVersionItem>,
}

#[derive(serde::Deserialize)]
pub struct SaveVersionItem {
    pub core_type: String,
    pub version: String,
    pub channel: Option<String>,
}

/// Persist the user's selection of upstream versions. Existing records are
/// skipped, so the call is idempotent.
pub async fn save_core_versions(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SaveVersionsPayload>,
) -> ApiResult<Value> {
    if payload.versions.is_empty() {
        return Err(ApiError::bad_request(
            "empty_selection",
            "no versions selected",
        ));
    }

    let mut added = 0u64;
    for item in payload.versions {
        let core_type = item
            .core_type
            .parse::<CoreType>()
            .map_err(|_| ApiError::bad_request("invalid_core_type", "unknown core type"))?;
        let version = item.version.trim().to_string();
        if version.is_empty() {
            return Err(ApiError::bad_request(
                "invalid_version",
                "version is required",
            ));
        }
        let channel = match item.channel.as_deref().unwrap_or(CHANNEL_RELEASE) {
            CHANNEL_RELEASE => CHANNEL_RELEASE,
            CHANNEL_PRERELEASE => CHANNEL_PRERELEASE,
            _ => {
                return Err(ApiError::bad_request(
                    "invalid_channel",
                    "channel must be 'release' or 'prerelease'",
                ));
            }
        };

        let exists = core_version::Entity::find()
            .filter(core_version::Column::CoreType.eq(core_type.to_string()))
            .filter(core_version::Column::Version.eq(&version))
            .one(&state.db)
            .await
            .map_err(ApiError::from)?
            .is_some();
        if exists {
            continue;
        }

        let active = core_version::ActiveModel {
            id: Set(Uuid::new_v4()),
            core_type: Set(core_type.to_string()),
            version: Set(version),
            channel: Set(channel.to_string()),
            created_at: Set(chrono::Utc::now().into()),
        };
        active.insert(&state.db).await.map_err(ApiError::from)?;
        added += 1;
    }

    Ok(ApiResponse::new(json!({ "added": added })))
}
