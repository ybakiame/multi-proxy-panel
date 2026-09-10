//! User-defined rule set market source fetch and cache management.
//!
//! 内置静态市场目录已取消，市场内容全部来自用户添加的远程 JSON 目录源。每个源
//! 的目录 JSON **原样**缓存到 `<data_dir>/rulesets/market/<source_id>.json`，供
//! 离线读取与更新判断；条目解析在读取时按结构逐项过滤非法项。
//!
//! 拉取复用 [`crate::fetch_resource_text`]（含 GitHub 代理前缀 / 走本地代理 / 重试）。

use std::collections::HashSet;
use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};

use super::{MarketSource, MarketSourceKind, RuleSetFormat};

/// Market cache root under data_dir (`rulesets/`).
pub const MARKET_ROOT_DIR: &str = "rulesets";
/// Market cache sub-directory under [`MARKET_ROOT_DIR`].
pub const MARKET_SUB_DIR: &str = "market";

/// One entry in a remote market catalog JSON array.
///
/// 目录格式为 `[{id, name, description, category, format, url}, ...]`；
/// `description` / `category` 允许缺省（默认空串），`id` / `name` / `format` /
/// `url` 为必需字段，缺一即视为非法项被过滤。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketEntry {
    /// Entry id (used as the added custom rule set tag).
    pub id: String,
    /// Display name (used as the added custom rule set name).
    pub name: String,
    /// Optional description.
    #[serde(default)]
    pub description: String,
    /// Optional category (used for market page filtering).
    #[serde(default)]
    pub category: String,
    /// Rule set file format (`binary` = `.srs`, `source` = JSON).
    pub format: RuleSetFormat,
    /// Rule set download URL.
    pub url: String,
    /// Remote modification time (Unix seconds; 0 = unknown).
    ///
    /// JSON 目录可显式提供；GitHub releases 源取自资产的 `updated_at`。添加为
    /// 规则集时写入 `custom_rule_sets.remote_updated_at`（比 HEAD 更准）。
    #[serde(default)]
    pub updated_at: u64,
}

/// Parse a remote market catalog JSON text into valid entries.
///
/// 顶层必须是 JSON 数组（否则报错，供「添加 / 刷新」验证失败不落盘）；数组内逐项
/// 解析，结构非法或 `id` / `url` 为空的条目被静默过滤。
pub fn parse_market_entries(text: &str) -> PanelResult<Vec<MarketEntry>> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| PanelError::Client(format!("market catalog is not valid JSON: {e}")))?;
    let Some(items) = value.as_array() else {
        return Err(PanelError::Client(
            "market catalog must be a JSON array".to_string(),
        ));
    };
    let entries = items
        .iter()
        .filter_map(|item| serde_json::from_value::<MarketEntry>(item.clone()).ok())
        .filter(|entry| !entry.id.trim().is_empty() && !entry.url.trim().is_empty())
        .collect();
    Ok(entries)
}

/// Build the GitHub releases API URL for `owner/repo`.
///
/// `tag` 为空 → `/releases/latest`；否则 → `/releases/tags/<tag>`。
pub fn github_releases_api_url(owner_repo: &str, tag: &str) -> String {
    let base = format!("https://api.github.com/repos/{owner_repo}/releases");
    let tag = tag.trim();
    if tag.is_empty() {
        format!("{base}/latest")
    } else {
        format!("{base}/tags/{tag}")
    }
}

/// Parse a GitHub release JSON response's `assets[]` into market entries.
///
/// 只接受 `.srs`（`binary`）与 `.json`（`source`）资产，去后缀作为 id/name；
/// `updated_at` 取资产的 ISO-8601 时间（转 Unix 秒，解析失败归 0）。非 JSON 或
/// 缺 `assets` 数组返回 `Err`（供「添加 / 刷新」验证失败不落盘）；数组内缺少
/// `name` / `browser_download_url` 或格式不符的资产被静默过滤。
pub fn parse_github_release_assets(text: &str) -> PanelResult<Vec<MarketEntry>> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| PanelError::Client(format!("github release is not valid JSON: {e}")))?;
    let Some(assets) = value.get("assets").and_then(|a| a.as_array()) else {
        return Err(PanelError::Client(
            "github release response has no assets array".to_string(),
        ));
    };
    let mut entries = Vec::new();
    for asset in assets {
        let Some(name) = asset.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str()) else {
            continue;
        };
        let (id, format) = if let Some(stem) = name.strip_suffix(".srs") {
            (stem, RuleSetFormat::Binary)
        } else if let Some(stem) = name.strip_suffix(".json") {
            (stem, RuleSetFormat::Source)
        } else {
            continue;
        };
        let id = id.trim();
        if id.is_empty() || url.trim().is_empty() {
            continue;
        }
        let updated_at = asset
            .get("updated_at")
            .and_then(|v| v.as_str())
            .and_then(parse_rfc3339_secs)
            .unwrap_or(0);
        entries.push(MarketEntry {
            id: id.to_string(),
            name: id.to_string(),
            description: String::new(),
            category: String::new(),
            format,
            url: url.to_string(),
            updated_at,
        });
    }
    Ok(entries)
}

/// Parse an RFC 3339 / ISO-8601 timestamp into Unix seconds (negative → 0).
fn parse_rfc3339_secs(value: &str) -> Option<u64> {
    chrono::DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp().max(0) as u64)
}

impl MarketSource {
    /// Build a JSON catalog source (existing behavior).
    pub fn json(id: String, name: String, url: String) -> Self {
        Self {
            id,
            name,
            url,
            kind: MarketSourceKind::Json,
            last_fetched: 0,
        }
    }

    /// Build a GitHub releases source; `url` is the resolved releases API URL.
    pub fn github(id: String, name: String, owner_repo: String, tag: String) -> Self {
        let url = github_releases_api_url(&owner_repo, &tag);
        Self {
            id,
            name,
            url,
            kind: MarketSourceKind::GithubReleases { owner_repo, tag },
            last_fetched: 0,
        }
    }

    /// Stable discriminator string for the frontend View contract.
    pub fn kind_str(&self) -> &'static str {
        match &self.kind {
            MarketSourceKind::Json => "json",
            MarketSourceKind::GithubReleases { .. } => "github_releases",
        }
    }
}

/// Manager for user-defined market source catalog cache.
#[derive(Debug, Clone)]
pub struct MarketManager {
    data_dir: PathBuf,
}

impl MarketManager {
    /// Create manager based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Market cache directory: `data_dir/rulesets/market/`.
    pub fn market_dir(&self) -> PathBuf {
        self.data_dir.join(MARKET_ROOT_DIR).join(MARKET_SUB_DIR)
    }

    /// Cache file path for a market source: `<market_dir>/<source_id>.json`.
    pub fn market_source_file_path(&self, source_id: &str) -> PathBuf {
        self.market_dir().join(format!("{source_id}.json"))
    }

    /// Fetch a remote catalog and parse it.
    ///
    /// Returns the raw JSON text plus the parsed valid entries. Network / parse
    /// failures return `Err`; the caller must not persist anything in that case
    /// (existing cache stays intact for refresh).
    pub async fn fetch_catalog(&self, url: &str) -> PanelResult<(String, Vec<MarketEntry>)> {
        let text =
            crate::fetch_resource_text(&self.data_dir, url, std::time::Duration::from_secs(60))
                .await?;
        let entries = parse_market_entries(&text)?;
        Ok((text, entries))
    }

    /// Fetch a GitHub release and parse its assets into market entries.
    ///
    /// Returns the **normalized entries JSON** (a `MarketEntry` array) plus the
    /// parsed entries, so the on-disk cache keeps one shape for both source
    /// kinds and `read_cache_entries` needs no kind awareness. `tag` empty →
    /// latest release. Network / parse failures return `Err` (nothing persisted).
    pub async fn fetch_github_release(
        &self,
        owner_repo: &str,
        tag: &str,
    ) -> PanelResult<(String, Vec<MarketEntry>)> {
        let url = github_releases_api_url(owner_repo, tag);
        let text =
            crate::fetch_resource_text(&self.data_dir, &url, std::time::Duration::from_secs(60))
                .await?;
        let entries = parse_github_release_assets(&text)?;
        let normalized = serde_json::to_string(&entries).map_err(|e| {
            PanelError::Client(format!("failed to serialize github release entries: {e}"))
        })?;
        Ok((normalized, entries))
    }

    /// Fetch a source's catalog according to its kind.
    ///
    /// Dispatches to [`Self::fetch_catalog`] (JSON) or [`Self::fetch_github_release`]
    /// (GitHub releases); returns the cache text + parsed entries.
    pub async fn fetch_for_source(
        &self,
        source: &MarketSource,
    ) -> PanelResult<(String, Vec<MarketEntry>)> {
        match &source.kind {
            MarketSourceKind::Json => self.fetch_catalog(&source.url).await,
            MarketSourceKind::GithubReleases { owner_repo, tag } => {
                self.fetch_github_release(owner_repo, tag).await
            }
        }
    }

    /// Persist the raw catalog JSON to `<market_dir>/<source_id>.json`.
    pub fn write_cache(&self, source_id: &str, raw: &str) -> PanelResult<()> {
        let path = self.market_source_file_path(source_id);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, raw)?;
        Ok(())
    }

    /// Read and parse a source's cached catalog.
    ///
    /// Missing / invalid cache yields an empty list. Pure read: never touches
    /// the network.
    pub fn read_cache_entries(&self, source_id: &str) -> Vec<MarketEntry> {
        let path = self.market_source_file_path(source_id);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        parse_market_entries(&text).unwrap_or_default()
    }

    /// Number of valid entries in a source's cached catalog.
    pub fn cached_entry_count(&self, source_id: &str) -> usize {
        self.read_cache_entries(source_id).len()
    }

    /// Remove a source's cache file (missing file is not an error).
    pub fn remove_cache(&self, source_id: &str) -> PanelResult<()> {
        let path = self.market_source_file_path(source_id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Best-effort cleanup of cache files whose source is no longer present.
    ///
    /// Mirrors the custom rule set orphan cleanup pattern.
    pub fn cleanup_caches(&self, keep_ids: &HashSet<String>) {
        let dir = self.market_dir();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Some(stem) = name.strip_suffix(".json") else {
                continue;
            };
            if !keep_ids.contains(stem)
                && let Err(e) = std::fs::remove_file(entry.path())
            {
                tracing::warn!(
                    path = %entry.path().display(),
                    error = %e,
                    "failed to remove orphaned market cache file"
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/market_tests.rs"]
mod tests;
