//! Rule set download, cache, and auto-update management.
//!
//! ADR-0002, section 3.4.

use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};

use super::{
    CustomRuleSet, CustomRuleSetSource, LocalOverride, LocalRuleSetRef, RuleSetFormat, RuleSetKind,
    RuleSetSource, RuleSetSubscription,
};

/// Rule set cache directory name under data_dir.
pub const RULE_SET_CACHE_DIR: &str = "rule_sets";

/// Custom rule set storage root under data_dir (`rulesets/custom/`).
///
/// Deliberately separate from the built-in community cache directory
/// (`rule_sets/`): user custom file names are `<id>.srs|json` and share no
/// namespace with community ids, so mixing the two roots would risk collisions.
pub const CUSTOM_RULE_SET_ROOT_DIR: &str = "rulesets";
/// Custom rule set sub-directory under [`CUSTOM_RULE_SET_ROOT_DIR`].
pub const CUSTOM_RULE_SET_SUB_DIR: &str = "custom";

/// Rule set manager handles download, cache, and subscription state.
#[derive(Debug, Clone)]
pub struct RuleSetManager {
    data_dir: PathBuf,
}

impl RuleSetManager {
    /// Create manager based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Cache directory: `data_dir/rule_sets/`.
    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join(RULE_SET_CACHE_DIR)
    }

    /// Cached file path for a community rule set (sing-box `.srs`).
    pub fn cache_file_path(&self, community_id: &str) -> PathBuf {
        self.cache_dir().join(format!("{community_id}.srs"))
    }

    /// Check if a cached file exists.
    pub fn is_cached(&self, community_id: &str) -> bool {
        self.cache_file_path(community_id).exists()
    }

    // -----------------------------------------------------------------------
    // Custom rule sets (user-defined: remote URL / manual JSON)
    // -----------------------------------------------------------------------

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

    /// Build custom rule set entries matching the given IDs (subset download).
    ///
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

    /// Toggle subscription state for a rule set.
    ///
    /// When `subscribed = true`, triggers a download attempt.
    /// Returns the updated subscription.
    pub async fn toggle_subscription(
        &self,
        ovr: &mut LocalOverride,
        community_id: &str,
        subscribed: bool,
    ) -> PanelResult<bool> {
        let Some(sub) = ovr
            .rule_set_subscriptions
            .iter_mut()
            .find(|s| s.community_id == community_id)
        else {
            return Err(PanelError::Client(format!(
                "rule set {community_id} not found"
            )));
        };

        let changed = sub.subscribed != subscribed;
        sub.subscribed = subscribed;

        if subscribed && changed {
            // Trigger download on subscribe.
            if let Err(e) = self.download_rule_set(sub).await {
                tracing::warn!(
                    community_id = %sub.community_id,
                    error = %e,
                    "rule set download failed on subscribe"
                );
                // Keep subscribed = true; download will retry on next update.
            }
        }

        Ok(changed)
    }

    /// Download a single rule set (sing-box `.srs` variant).
    ///
    /// - On failure, preserves existing cache (graceful degradation).
    /// - Updates `last_updated` timestamp on success.
    pub async fn download_rule_set(&self, sub: &RuleSetSubscription) -> PanelResult<()> {
        let cache_dir = self.cache_dir();
        std::fs::create_dir_all(&cache_dir)?;

        let singbox_url = sub.singbox_url_template.replace("{tag}", &sub.community_id);
        let singbox_path = cache_dir.join(format!("{}.srs", sub.community_id));
        if let Err(e) = self.download_file(&singbox_url, &singbox_path).await {
            tracing::warn!(
                community_id = %sub.community_id,
                url = %singbox_url,
                error = %e,
                "sing-box rule set download failed, keeping old cache if any"
            );
        }

        Ok(())
    }

    /// Update all subscribed rule sets now.
    ///
    /// Iterates all subscribed community rule sets and all **enabled Remote
    /// custom rule sets**, attempting download of each. Logs warnings for
    /// individual failures but does not fail the batch. Manual custom rule
    /// sets have no remote source and are skipped.
    pub async fn update_all_subscribed(&self, ovr: &mut LocalOverride) -> PanelResult<usize> {
        let mut updated = 0;
        for sub in &ovr.rule_set_subscriptions {
            if !sub.subscribed {
                continue;
            }
            match self.download_rule_set(sub).await {
                Ok(()) => updated += 1,
                Err(e) => {
                    tracing::warn!(
                        community_id = %sub.community_id,
                        error = %e,
                        "rule set update failed"
                    );
                }
            }
        }
        let now_sec = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        updated += self
            .download_custom_many(
                ovr.custom_rule_sets.iter_mut().filter(|rs| {
                    rs.enabled && matches!(rs.source, CustomRuleSetSource::Remote { .. })
                }),
                now_sec,
            )
            .await;
        Ok(updated)
    }

    /// Build [`LocalRuleSetRef`] entries from subscribed rule sets.
    ///
    /// Only includes subscribed rule sets that have a cached file.
    pub fn build_rule_set_refs(&self, ovr: &LocalOverride) -> Vec<LocalRuleSetRef> {
        let mut refs = Vec::new();
        for sub in &ovr.rule_set_subscriptions {
            if !sub.subscribed {
                continue;
            }
            let cache_path = self.cache_file_path(&sub.community_id);
            if !cache_path.exists() {
                tracing::debug!(
                    community_id = %sub.community_id,
                    "rule set cache missing, skipping"
                );
                continue;
            }

            let (kind, source) = (
                RuleSetKind::SingBoxRemote,
                RuleSetSource::Local {
                    path: cache_path.to_string_lossy().to_string(),
                },
            );

            refs.push(LocalRuleSetRef {
                id: format!("rs-ref-{}", sub.community_id),
                name: sub.display_name.clone(),
                tag: sub.community_id.clone(),
                kind,
                source,
                enabled: true,
                auto_update_interval_minutes: sub.default_interval_minutes,
                last_updated: 0, // Could be enhanced to read file mtime.
            });
        }
        refs
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

/// Rule set status view for frontend display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleSetStatusView {
    pub id: String,
    pub community_id: String,
    pub display_name: String,
    pub category: String,
    pub subscribed: bool,
    pub singbox_cached: bool,
    pub last_updated: u64,
}

impl RuleSetStatusView {
    /// Build status view from subscription + manager cache state.
    pub fn from_subscription(sub: &RuleSetSubscription, manager: &RuleSetManager) -> Self {
        Self {
            id: sub.id.clone(),
            community_id: sub.community_id.clone(),
            display_name: sub.display_name.clone(),
            category: format!("{:?}", sub.category).to_lowercase(),
            subscribed: sub.subscribed,
            singbox_cached: manager.is_cached(&sub.community_id),
            last_updated: 0, // TODO: read from file mtime or stored timestamp.
        }
    }
}

#[cfg(test)]
#[path = "tests/ruleset_tests.rs"]
mod tests;
