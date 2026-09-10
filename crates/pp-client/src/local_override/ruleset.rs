//! Rule set download and cache management for user-defined custom rule sets.
//!
//! 自「废弃内置规则集订阅」起只管理用户自控的 [`CustomRuleSet`]（remote URL
//! 下载 / manual JSON 落盘）；内置社区订阅的下载/缓存/引用生成已全部移除。
//!
//! ADR-0002, section 3.4.

use std::path::PathBuf;

use pp_common::PanelResult;

use super::{CustomRuleSet, CustomRuleSetSource, LocalOverride, RuleSetFormat};

/// Custom rule set storage root under data_dir (`rulesets/custom/`).
pub const CUSTOM_RULE_SET_ROOT_DIR: &str = "rulesets";
/// Custom rule set sub-directory under [`CUSTOM_RULE_SET_ROOT_DIR`].
pub const CUSTOM_RULE_SET_SUB_DIR: &str = "custom";

/// Result of updating a single custom rule set (smart skip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSetUpdateStatus {
    /// Remote had newer content (or no `Last-Modified`): downloaded,
    /// `last_updated` bumped to the download completion time.
    Updated,
    /// Remote `Last-Modified` ≤ local `last_updated`: download skipped.
    Skipped,
    /// HEAD or download failed (best-effort; never fatal to a batch).
    Failed,
}

/// Aggregated outcome of updating one or more custom rule sets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RuleSetUpdateOutcome {
    /// Remote had newer content: downloaded + `last_updated` bumped.
    pub updated: usize,
    /// Remote `Last-Modified` ≤ local `last_updated`: download skipped.
    pub skipped: usize,
    /// HEAD / download failed (best-effort, per-entry).
    pub failed: usize,
}

/// Rule set manager handles download and file sync of custom rule sets.
#[derive(Debug, Clone)]
pub struct RuleSetManager {
    data_dir: PathBuf,
}

impl RuleSetManager {
    /// Create manager based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Custom rule set storage directory: `data_dir/rulesets/custom/`.
    pub fn custom_rule_set_dir(&self) -> PathBuf {
        self.data_dir
            .join(CUSTOM_RULE_SET_ROOT_DIR)
            .join(CUSTOM_RULE_SET_SUB_DIR)
    }

    /// Backing file path for a custom rule set:
    /// `<custom_dir>/<id>.srs` (binary) or `<custom_dir>/<id>.json` (source).
    pub fn custom_rule_set_file_path(&self, id: &str, format: RuleSetFormat) -> PathBuf {
        let ext = match format {
            RuleSetFormat::Source => "json",
            RuleSetFormat::Binary => "srs",
        };
        self.custom_rule_set_dir().join(format!("{id}.{ext}"))
    }

    /// Whether the backing file of a custom rule set exists on disk.
    ///
    /// - Remote: cache file for its declared format must exist.
    /// - Manual: the persisted `<id>.json` file must exist.
    pub fn has_custom_rule_set_file(&self, rs: &CustomRuleSet) -> bool {
        let format = rs.file_format();
        self.custom_rule_set_file_path(&rs.id, format).exists()
    }

    /// Persist Manual custom rule set contents to `<custom_dir>/<id>.json`
    /// and best-effort remove backing files of custom rule sets that are no
    /// longer present in `custom_sets` (deleted rule sets or format switches).
    ///
    /// Called on save so the injected local rule_set paths always resolve.
    pub fn sync_custom_rule_set_files(&self, custom_sets: &[CustomRuleSet]) -> PanelResult<()> {
        let dir = self.custom_rule_set_dir();
        std::fs::create_dir_all(&dir)?;

        // Collect the exact file names the given list should keep on disk.
        let keep: std::collections::HashSet<String> = custom_sets
            .iter()
            .map(|rs| {
                let path = self.custom_rule_set_file_path(&rs.id, rs.file_format());
                path.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        // Write manual contents (overwrites on re-save with edited content).
        for rs in custom_sets {
            if let CustomRuleSetSource::Manual { content } = &rs.source {
                let path = self.custom_rule_set_file_path(&rs.id, RuleSetFormat::Source);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, content)?;
            }
        }

        // Best-effort cleanup of orphaned backing files (deleted rule sets and
        // stale files left behind by a source kind/format switch).
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !keep.contains(&name)
                    && let Err(e) = std::fs::remove_file(entry.path())
                {
                    tracing::warn!(
                        path = %entry.path().display(),
                        error = %e,
                        "failed to remove orphaned custom rule set file"
                    );
                }
            }
        }

        Ok(())
    }

    /// Download a Remote custom rule set to `<custom_dir>/<id>.<ext>`.
    ///
    /// No-op for Manual sources. On failure preserves any existing file.
    pub async fn download_custom_rule_set(&self, rs: &CustomRuleSet) -> PanelResult<()> {
        let CustomRuleSetSource::Remote { url, format } = &rs.source else {
            return Ok(());
        };
        let path = self.custom_rule_set_file_path(&rs.id, *format);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.download_file(url, &path).await
    }

    /// Update a single Remote custom rule set with smart skip.
    ///
    /// 先发 HEAD 取 `Last-Modified`：
    /// - 有且 `≤ last_updated`（且已下载过）→ 跳过下载，仅刷新 `remote_updated_at`；
    /// - 有且更新 → 下载，`last_updated` 取下载完成时间、`remote_updated_at` 取头值；
    /// - 无头 → 直接下载，`remote_updated_at` 归 0；
    /// - HEAD / 下载失败 → [`RuleSetUpdateStatus::Failed`]（告警，不 panic）。
    ///
    /// Manual 无远端源，直接 [`RuleSetUpdateStatus::Skipped`]。
    pub async fn update_custom_rule_set(&self, rs: &mut CustomRuleSet) -> RuleSetUpdateStatus {
        let url = match &rs.source {
            CustomRuleSetSource::Remote { url, .. } => url.clone(),
            CustomRuleSetSource::Manual { .. } => return RuleSetUpdateStatus::Skipped,
        };

        let remote_lm = match crate::fetch_resource_last_modified(
            &self.data_dir,
            &url,
            std::time::Duration::from_secs(60),
        )
        .await
        {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    id = %rs.id,
                    tag = %rs.tag,
                    error = %e,
                    "custom rule set HEAD failed"
                );
                return RuleSetUpdateStatus::Failed;
            }
        };

        match remote_lm {
            Some(lm) => {
                rs.remote_updated_at = lm;
                // 已下载过且远端未更新 → 跳过（`last_updated` 为下载完成时间）。
                if rs.last_updated > 0 && lm <= rs.last_updated {
                    return RuleSetUpdateStatus::Skipped;
                }
            }
            None => rs.remote_updated_at = 0,
        }

        match self.download_custom_rule_set(rs).await {
            Ok(()) => {
                rs.last_updated = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                RuleSetUpdateStatus::Updated
            }
            Err(e) => {
                tracing::warn!(
                    id = %rs.id,
                    tag = %rs.tag,
                    error = %e,
                    "custom rule set update failed"
                );
                RuleSetUpdateStatus::Failed
            }
        }
    }

    /// Manually update all Remote custom rule sets now (smart skip).
    ///
    /// 对每个 custom Remote 走 [`Self::update_custom_rule_set`]（HEAD 智能跳过），
    /// 汇总 updated / skipped / failed。命令层同步 await 全部完成后才返回，保证前端
    /// invalidate 重拉 `local_override_get` 时即可看到 `cached` / `last_updated` /
    /// `remote_updated_at` 变化。Manual 无远端源不计入；单个失败仅告警，不影响整批
    /// （best-effort）。
    pub async fn update_custom_remotes(
        &self,
        ovr: &mut LocalOverride,
    ) -> PanelResult<RuleSetUpdateOutcome> {
        let mut outcome = RuleSetUpdateOutcome::default();
        for rs in ovr
            .custom_rule_sets
            .iter_mut()
            .filter(|rs| matches!(rs.source, CustomRuleSetSource::Remote { .. }))
        {
            match self.update_custom_rule_set(rs).await {
                RuleSetUpdateStatus::Updated => outcome.updated += 1,
                RuleSetUpdateStatus::Skipped => outcome.skipped += 1,
                RuleSetUpdateStatus::Failed => outcome.failed += 1,
            }
        }
        Ok(outcome)
    }

    /// Download a file from URL to path.
    async fn download_file(&self, url: &str, path: &std::path::Path) -> PanelResult<()> {
        let bytes =
            crate::fetch_resource_bytes(&self.data_dir, url, std::time::Duration::from_secs(60))
                .await?;
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/ruleset_tests.rs"]
mod tests;
