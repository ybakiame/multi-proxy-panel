//! Local Override storage: reads/writes `data_dir/local_override.json`.
//!
//! Follows the same resilience pattern as [`ProfileStoreV2`]:
//! - Missing file → default (empty) config.
//! - Corrupted file → log warning, fall back to default (non-blocking).
//! - `#[serde(default)]` on all fields for forward compatibility.

use std::path::PathBuf;

use pp_common::PanelResult;

use super::{LocalOverride, RuleSetSubscription};

/// Storage for `LocalOverride` at `data_dir/local_override.json`.
#[derive(Debug, Clone)]
pub struct LocalOverrideStore {
    data_dir: PathBuf,
}

impl LocalOverrideStore {
    /// Create storage based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/local_override.json`.
    pub fn override_file(&self) -> PathBuf {
        self.data_dir.join("local_override.json")
    }

    /// Load local override config.
    ///
    /// - File missing → returns default (empty) config with enabled = true.
    /// - File corrupted → logs warning, returns default (does not block startup).
    pub fn load(&self) -> PanelResult<LocalOverride> {
        let path = self.override_file();
        if !path.exists() {
            return Ok(LocalOverride::default());
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local_override.json unreadable, fall back to default"
                );
                return Ok(LocalOverride::default());
            }
        };
        match serde_json::from_str(&text) {
            Ok(ovr) => Ok(ovr),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local_override.json corrupted, fall back to default"
                );
                Ok(LocalOverride::default())
            }
        }
    }

    /// Save local override config to `data_dir/local_override.json`.
    pub fn save(&self, ovr: &LocalOverride) -> PanelResult<()> {
        let path = self.override_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(ovr)?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    /// Initialize built-in rule set subscriptions when the list is empty.
    ///
    /// Called on first access to ensure the built-in 5 rule sets are available.
    pub fn ensure_builtin_subscriptions(&self) -> PanelResult<LocalOverride> {
        let mut ovr = self.load()?;
        if ovr.rule_set_subscriptions.is_empty() {
            ovr.rule_set_subscriptions = built_in_rule_set_subscriptions();
            self.save(&ovr)?;
        }
        Ok(ovr)
    }
}

/// Built-in community rule set subscriptions (ADR-0002, section 3.4.1).
///
/// | community_id | display_name | category | singbox_url_template | mihomo_url_template |
pub fn built_in_rule_set_subscriptions() -> Vec<RuleSetSubscription> {
    vec![
        RuleSetSubscription {
            id: "sub-geoip-cn".to_string(),
            community_id: "geoip-cn".to_string(),
            display_name: "GeoIP China".to_string(),
            category: super::RuleSetCategory::Geoip,
            subscribed: false,
            singbox_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/ip/{tag}.srs".to_string(),
            mihomo_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/ip/{tag}.yaml".to_string(),
            default_interval_minutes: 1440,
        },
        RuleSetSubscription {
            id: "sub-geosite-cn".to_string(),
            community_id: "geosite-cn".to_string(),
            display_name: "GeoSite China".to_string(),
            category: super::RuleSetCategory::Geosite,
            subscribed: false,
            singbox_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/geosite/{tag}.srs".to_string(),
            mihomo_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/geosite/{tag}.yaml".to_string(),
            default_interval_minutes: 1440,
        },
        RuleSetSubscription {
            id: "sub-geosite-ads".to_string(),
            community_id: "geosite-ads".to_string(),
            display_name: "Ad Domains".to_string(),
            category: super::RuleSetCategory::Ads,
            subscribed: false,
            singbox_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/geosite/category-ads-all.srs".to_string(),
            mihomo_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/geosite/category-ads-all.yaml".to_string(),
            default_interval_minutes: 1440,
        },
        RuleSetSubscription {
            id: "sub-geosite-geolocation-not-cn".to_string(),
            community_id: "geosite-geolocation-!cn".to_string(),
            display_name: "Non-China Geolocation".to_string(),
            category: super::RuleSetCategory::Geosite,
            subscribed: false,
            singbox_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo/geosite/{tag}.srs".to_string(),
            mihomo_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo/geosite/{tag}.yaml".to_string(),
            default_interval_minutes: 1440,
        },
        RuleSetSubscription {
            id: "sub-geoip-private".to_string(),
            community_id: "geoip-private".to_string(),
            display_name: "Private IP Ranges".to_string(),
            category: super::RuleSetCategory::Geoip,
            subscribed: false,
            singbox_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/ip/{tag}.srs".to_string(),
            mihomo_url_template: "https://github.com/MetaCubeX/meta-rules-dat/raw/meta/geo-lite/ip/{tag}.yaml".to_string(),
            default_interval_minutes: 1440,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let ovr = store.load().unwrap();
        assert!(ovr.singbox.rules.is_empty());
        assert!(ovr.mihomo.rules.is_empty());
        assert!(ovr.rule_set_subscriptions.is_empty());
        assert!(ovr.applied_templates.is_empty());
        assert!(ovr.singbox.enabled);
        assert!(ovr.mihomo.enabled);
    }

    #[test]
    fn store_load_corrupted_file_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        std::fs::write(store.override_file(), "not valid json {{{").unwrap();
        let ovr = store.load().unwrap();
        // Should fall back to default, not panic/error.
        assert!(ovr.singbox.rules.is_empty());
        assert!(ovr.singbox.enabled);
    }

    #[test]
    fn store_save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let mut ovr = LocalOverride::default();
        ovr.singbox.enabled = false;
        ovr.singbox.rules.push(super::super::LocalRule {
            id: "r1".to_string(),
            name: "test".to_string(),
            enabled: true,
            match_type: super::super::RuleMatchType::Domain,
            target: "example.com".to_string(),
            action: super::super::RuleAction::Direct,
            advanced: Default::default(),
            note: String::new(),
            created_at: 1,
            sort_order: 0,
        });
        store.save(&ovr).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(ovr, loaded);
    }

    #[test]
    fn ensure_builtin_initializes_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let ovr = store.ensure_builtin_subscriptions().unwrap();
        assert_eq!(ovr.rule_set_subscriptions.len(), 5);
        // Second call should not duplicate.
        let ovr2 = store.ensure_builtin_subscriptions().unwrap();
        assert_eq!(ovr2.rule_set_subscriptions.len(), 5);
    }

    #[test]
    fn built_in_has_five_rule_sets() {
        let subs = built_in_rule_set_subscriptions();
        assert_eq!(subs.len(), 5);
        let ids: Vec<_> = subs.iter().map(|s| s.community_id.as_str()).collect();
        assert!(ids.contains(&"geoip-cn"));
        assert!(ids.contains(&"geosite-cn"));
        assert!(ids.contains(&"geosite-ads"));
        assert!(ids.contains(&"geosite-geolocation-!cn"));
        assert!(ids.contains(&"geoip-private"));
    }
}
