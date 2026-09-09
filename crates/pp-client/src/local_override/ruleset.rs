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

    /// Download the given custom rule sets, bumping `last_updated` on success.
    ///
    /// Failures are logged individually and never fail the batch (best-effort).
    /// Returns the number of successfully downloaded rule sets.
    async fn download_custom_many(
        &self,
        sets: impl Iterator<Item = &mut CustomRuleSet>,
        now_sec: u64,
    ) -> usize {
        let mut updated = 0;
        for rs in sets {
            match self.download_custom_rule_set(rs).await {
                Ok(()) => {
                    rs.last_updated = now_sec;
                    updated += 1;
                }
                Err(e) => {
                    tracing::warn!(
                        id = %rs.id,
                        tag = %rs.tag,
                        error = %e,
                        "custom rule set update failed"
                    );
                }
            }
        }
        updated
    }

    /// Manually update all Remote custom rule sets now.
    ///
    /// 自「规则集移除 enabled」起不再有启用过滤：纯资源管理语义下「立即更新」
    /// 刷新**全部** custom Remote（下载失败条目 best-effort 跳过）。
    /// 命令层**同步 await** 每个下载完成后才返回，保证前端 invalidate 重拉
    /// `local_override_get` 时即可看到 `cached` / `last_updated` 变化（用户反馈
    /// 的“缓存状态和更新时间不刷新”修复）。Manual 无远端源被跳过；单个失败仅
    /// 告警，不影响整批（best-effort）。
    pub async fn update_custom_remotes(&self, ovr: &mut LocalOverride) -> PanelResult<usize> {
        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let updated = self
            .download_custom_many(
                ovr.custom_rule_sets
                    .iter_mut()
                    .filter(|rs| matches!(rs.source, CustomRuleSetSource::Remote { .. })),
                now_sec,
            )
            .await;
        Ok(updated)
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
