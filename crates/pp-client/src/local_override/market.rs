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

use super::RuleSetFormat;

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
