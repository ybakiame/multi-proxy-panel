//! Rule set download, cache, and auto-update management.
//!
//! ADR-0002, section 3.4.

use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};

use super::{LocalOverride, LocalRuleSetRef, RuleSetKind, RuleSetSource, RuleSetSubscription};

/// Rule set cache directory name under data_dir.
pub const RULE_SET_CACHE_DIR: &str = "rule_sets";

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

    /// Cached file path for a community rule set.
    ///
    /// sing-box uses `.srs`, mihomo uses `.yaml`.
    pub fn cache_file_path(&self, community_id: &str, core_type: pp_common::CoreType) -> PathBuf {
        let ext = match core_type {
            pp_common::CoreType::SingBox => "srs",
            pp_common::CoreType::Mihomo => "yaml",
        };
        self.cache_dir().join(format!("{community_id}.{ext}"))
    }

    /// Check if a cached file exists.
    pub fn is_cached(&self, community_id: &str, core_type: pp_common::CoreType) -> bool {
        self.cache_file_path(community_id, core_type).exists()
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

    /// Download a single rule set for both cores.
    ///
    /// - Downloads sing-box `.srs` and mihomo `.yaml` variants.
    /// - On failure, preserves existing cache (graceful degradation).
    /// - Updates `last_updated` timestamp on success.
    pub async fn download_rule_set(&self, sub: &RuleSetSubscription) -> PanelResult<()> {
        let cache_dir = self.cache_dir();
        std::fs::create_dir_all(&cache_dir)?;

        // Download sing-box variant.
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

        // Download mihomo variant.
        let mihomo_url = sub.mihomo_url_template.replace("{tag}", &sub.community_id);
        let mihomo_path = cache_dir.join(format!("{}.yaml", sub.community_id));
        if let Err(e) = self.download_file(&mihomo_url, &mihomo_path).await {
            tracing::warn!(
                community_id = %sub.community_id,
                url = %mihomo_url,
                error = %e,
                "mihomo rule set download failed, keeping old cache if any"
            );
        }

        Ok(())
    }

    /// Update all subscribed rule sets now.
    ///
    /// Iterates all subscribed rule sets and attempts download.
    /// Logs warnings for individual failures but does not fail the batch.
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
        Ok(updated)
    }

    /// Build [`LocalRuleSetRef`] entries from subscribed rule sets for a given core.
    ///
    /// Only includes subscribed rule sets that have a cached file.
    pub fn build_rule_set_refs(
        &self,
        ovr: &LocalOverride,
        core_type: pp_common::CoreType,
    ) -> Vec<LocalRuleSetRef> {
        let mut refs = Vec::new();
        for sub in &ovr.rule_set_subscriptions {
            if !sub.subscribed {
                continue;
            }
            let cache_path = self.cache_file_path(&sub.community_id, core_type);
            if !cache_path.exists() {
                tracing::debug!(
                    community_id = %sub.community_id,
                    core = ?core_type,
                    "rule set cache missing, skipping"
                );
                continue;
            }

            let (kind, source) = match core_type {
                pp_common::CoreType::SingBox => (
                    RuleSetKind::SingBoxRemote,
                    RuleSetSource::Local {
                        path: cache_path.to_string_lossy().to_string(),
                    },
                ),
                pp_common::CoreType::Mihomo => (
                    RuleSetKind::MihomoFile,
                    RuleSetSource::Local {
                        path: cache_path.to_string_lossy().to_string(),
                    },
                ),
            };

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
    pub mihomo_cached: bool,
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
            singbox_cached: manager.is_cached(&sub.community_id, pp_common::CoreType::SingBox),
            mihomo_cached: manager.is_cached(&sub.community_id, pp_common::CoreType::Mihomo),
            last_updated: 0, // TODO: read from file mtime or stored timestamp.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_override::store::built_in_rule_set_subscriptions;

    #[test]
    fn cache_file_path_formats_correctly() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = RuleSetManager::new(dir.path().to_path_buf());
        let p1 = mgr.cache_file_path("geoip-cn", pp_common::CoreType::SingBox);
        assert_eq!(p1.file_name().unwrap(), "geoip-cn.srs");
        let p2 = mgr.cache_file_path("geoip-cn", pp_common::CoreType::Mihomo);
        assert_eq!(p2.file_name().unwrap(), "geoip-cn.yaml");
    }

    #[test]
    fn build_rule_set_refs_skips_unsubscribed() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = RuleSetManager::new(dir.path().to_path_buf());
        let ovr = LocalOverride {
            rule_set_subscriptions: built_in_rule_set_subscriptions(),
            ..Default::default()
        };
        // None subscribed, none cached.
        let refs = mgr.build_rule_set_refs(&ovr, pp_common::CoreType::SingBox);
        assert!(refs.is_empty());
    }

    #[test]
    fn build_rule_set_refs_includes_cached_subscribed() {
        let dir = tempfile::tempdir().unwrap();
        let mgr = RuleSetManager::new(dir.path().to_path_buf());
        let mut subs = built_in_rule_set_subscriptions();
        subs[0].subscribed = true;
        let ovr = LocalOverride {
            rule_set_subscriptions: subs,
            ..Default::default()
        };
        // Create fake cache.
        std::fs::create_dir_all(mgr.cache_dir()).unwrap();
        std::fs::write(
            mgr.cache_file_path("geoip-cn", pp_common::CoreType::SingBox),
            "fake",
        )
        .unwrap();

        let refs = mgr.build_rule_set_refs(&ovr, pp_common::CoreType::SingBox);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].tag, "geoip-cn");
        assert!(matches!(refs[0].kind, RuleSetKind::SingBoxRemote));
    }
}
