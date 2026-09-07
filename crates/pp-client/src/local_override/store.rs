//! Local Override storage: reads/writes `data_dir/local_override.json`.
//!
//! Follows the same resilience pattern as [`ProfileStoreV2`]:
//! - Missing file → default (empty) config.
//! - Corrupted file → log warning, fall back to default (non-blocking).
//! - `#[serde(default)]` on all fields for forward compatibility.
//!
//! 自「废弃内置规则集订阅与内置模板」起，`load` 内嵌**幂等存量迁移**：把旧版
//! 已订阅内置条目转换为用户自控的 custom Remote 规则集、清理内置模板记录，
//! 使旧文件一次读取即归一化到纯用户自控模型。

use std::path::PathBuf;

use pp_common::PanelResult;

use super::{CustomRuleSet, CustomRuleSetSource, LocalOverride, RuleSetFormat};
use crate::local_override::template::CUSTOM_TEMPLATE_PREFIX;

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
    /// - Legacy file → runs the idempotent built-in mechanism migration and
    ///   best-effort persists the normalized state back to disk.
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
            Ok(mut ovr) => {
                if migrate_legacy_builtins(&mut ovr) {
                    // 迁移结果写回磁盘，保证后续 load 读到已归一化数据（幂等）。
                    if let Err(e) = self.save(&ovr) {
                        tracing::warn!(
                            path = %path.display(),
                            error = %e,
                            "failed to persist legacy migration result"
                        );
                    }
                }
                Ok(ovr)
            }
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
}

/// 存量迁移（幂等）：把旧版内置机制数据归一化为纯用户自控模型。
///
/// 1. `rule_set_subscriptions` 中 `subscribed == true` 的条目 → 转换为启用的
///    [`CustomRuleSet`] Remote（Binary），随后清空整个订阅段（写回空数组
///    无害）。映射：
///    - `id`：新生成 UUID；
///    - `name` = `display_name`；
///    - `tag` = `community_id`；
///    - `source` = `Remote { url: singbox_url_template 的 "{tag}" 替换为
///      community_id, format: Binary }`；
///    - `enabled: true`、`last_updated: 0`（内置机制无此记录）。
/// 2. `applied_templates` 中 `template_id` **不带** `custom:` 前缀的记录删除
///    （内置模板撤销入口随模板移除）；这些记录生成的规则**保留**在
///    `singbox.rules`，由用户手动管理。
///
/// 返回是否发生了改动；未改动时 `load` 不会触发写盘。
fn migrate_legacy_builtins(ovr: &mut LocalOverride) -> bool {
    let mut changed = false;

    if !ovr.rule_set_subscriptions.is_empty() {
        for sub in ovr.rule_set_subscriptions.iter().filter(|s| s.subscribed) {
            let url = sub.singbox_url_template.replace("{tag}", &sub.community_id);
            ovr.custom_rule_sets.push(CustomRuleSet {
                id: uuid::Uuid::new_v4().to_string(),
                name: sub.display_name.clone(),
                tag: sub.community_id.clone(),
                source: CustomRuleSetSource::Remote {
                    url,
                    format: RuleSetFormat::Binary,
                },
                enabled: true,
                last_updated: 0,
            });
        }
        ovr.rule_set_subscriptions.clear();
        changed = true;
    }

    let before = ovr.applied_templates.len();
    ovr.applied_templates
        .retain(|t| t.template_id.starts_with(CUSTOM_TEMPLATE_PREFIX));
    changed |= ovr.applied_templates.len() != before;

    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_override::RuleSetSubscription;

    #[test]
    fn store_load_missing_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let ovr = store.load().unwrap();
        assert!(ovr.singbox.rules.is_empty());
        assert!(ovr.rule_set_subscriptions.is_empty());
        assert!(ovr.applied_templates.is_empty());
        assert!(ovr.singbox.enabled);
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

    /// 构造一份旧版 `local_override.json`（内置订阅 + 内置/自定义模板记录）。
    fn legacy_sub(name: &str, community_id: &str, url_tpl: &str) -> RuleSetSubscription {
        RuleSetSubscription {
            id: format!("sub-{name}"),
            community_id: community_id.to_string(),
            display_name: format!("Display {name}"),
            category: super::super::RuleSetCategory::Geosite,
            subscribed: true,
            singbox_url_template: url_tpl.to_string(),
            default_interval_minutes: 1440,
        }
    }

    fn write_legacy_file(dir: &tempfile::TempDir, json: serde_json::Value) {
        let path = dir.path().join("local_override.json");
        std::fs::write(path, serde_json::to_string_pretty(&json).unwrap()).unwrap();
    }

    #[test]
    fn migration_converts_subscribed_builtin_to_custom_remote() {
        let dir = tempfile::tempdir().unwrap();
        // {tag} 占位模板 + 无占位模板各一，验证 url 替换。
        let subs = vec![
            legacy_sub("a", "geoip-cn", "https://example.com/ip/{tag}.srs"),
            legacy_sub(
                "b",
                "geosite-ads",
                "https://example.com/geo/category-ads-all.srs",
            ),
        ];
        write_legacy_file(
            &dir,
            serde_json::json!({
                "singbox": { "rules": [], "rule_sets": [], "enabled": true },
                "rule_set_subscriptions": subs,
                "applied_templates": [
                    { "template_id": "return-china", "applied_at": 1, "generated_rule_ids": ["r1", "r2"] },
                    { "template_id": "custom:tpl-1", "applied_at": 2, "generated_rule_ids": ["r3"] }
                ],
                "custom_rule_sets": [],
                "custom_templates": []
            }),
        );

        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let ovr = store.load().unwrap();

        // 订阅段已清空。
        assert!(ovr.rule_set_subscriptions.is_empty());

        // 转换出的 custom Remote 字段映射逐一断言。
        assert_eq!(ovr.custom_rule_sets.len(), 2);
        let geoip = ovr
            .custom_rule_sets
            .iter()
            .find(|rs| rs.tag == "geoip-cn")
            .unwrap();
        assert_eq!(geoip.name, "Display a");
        uuid::Uuid::parse_str(&geoip.id).expect("migrated id must be a UUID");
        assert!(ovr.custom_rule_sets.iter().all(|rs| rs.enabled));
        assert!(ovr.custom_rule_sets.iter().all(|rs| rs.last_updated == 0));
        match &geoip.source {
            CustomRuleSetSource::Remote { url, format } => {
                assert_eq!(*format, RuleSetFormat::Binary);
                assert_eq!(url, "https://example.com/ip/geoip-cn.srs");
            }
            other => panic!("unexpected source {other:?}"),
        }
        let ads = ovr
            .custom_rule_sets
            .iter()
            .find(|rs| rs.tag == "geosite-ads")
            .unwrap();
        match &ads.source {
            CustomRuleSetSource::Remote { url, format } => {
                assert_eq!(*format, RuleSetFormat::Binary);
                assert_eq!(url, "https://example.com/geo/category-ads-all.srs");
            }
            other => panic!("unexpected source {other:?}"),
        }

        // 仅内置 applied 记录被删除，custom: 记录保留。
        assert_eq!(ovr.applied_templates.len(), 1);
        assert_eq!(ovr.applied_templates[0].template_id, "custom:tpl-1");

        // 迁移结果已写回磁盘：再次 load 幂等，不重复追加。
        let ovr2 = store.load().unwrap();
        assert_eq!(ovr2.custom_rule_sets.len(), 2);
        assert!(ovr2.rule_set_subscriptions.is_empty());
    }

    #[test]
    fn migration_ignores_unsubscribed_and_keeps_rules() {
        let dir = tempfile::tempdir().unwrap();
        // 旧文件：内置列表全部未订阅 + 一条内置规则记录 + 保留规则。
        let mut sub = legacy_sub("a", "geoip-cn", "https://example.com/ip/{tag}.srs");
        sub.subscribed = false;
        write_legacy_file(
            &dir,
            serde_json::json!({
                "singbox": {
                    "enabled": true,
                    "rule_sets": [],
                    "rules": [{
                        "id": "gen-1", "name": "GeoIP CN direct", "enabled": true,
                        "match_type": "rule_set", "target": "geoip-cn",
                        "action": "direct", "note": "", "created_at": 1, "sort_order": -100
                    }]
                },
                "rule_set_subscriptions": [sub],
                "applied_templates": [
                    { "template_id": "return-china", "applied_at": 1, "generated_rule_ids": ["gen-1"] }
                ],
                "custom_rule_sets": [],
                "custom_templates": []
            }),
        );

        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let ovr = store.load().unwrap();

        // 未订阅 → 不转换 custom；订阅段清空。
        assert!(ovr.custom_rule_sets.is_empty());
        assert!(ovr.rule_set_subscriptions.is_empty());
        // 内置模板生成的规则保留在 singbox.rules，用户可手动管理。
        assert_eq!(ovr.singbox.rules.len(), 1);
        assert_eq!(ovr.singbox.rules[0].target, "geoip-cn");
        // 内置 applied 记录删除。
        assert!(ovr.applied_templates.is_empty());
    }

    #[test]
    fn migration_is_idempotent_across_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let subs = vec![legacy_sub(
            "a",
            "geoip-cn",
            "https://example.com/ip/{tag}.srs",
        )];
        write_legacy_file(
            &dir,
            serde_json::json!({
                "singbox": { "rules": [], "rule_sets": [], "enabled": true },
                "rule_set_subscriptions": subs,
                "applied_templates": [],
                "custom_rule_sets": [],
                "custom_templates": []
            }),
        );

        let store = LocalOverrideStore::new(dir.path().to_path_buf());
        let first = store.load().unwrap();
        assert_eq!(first.custom_rule_sets.len(), 1);
        // 二次 load（此时文件已归一化）不重复转换。
        let second = store.load().unwrap();
        assert_eq!(second.custom_rule_sets.len(), 1);
        assert!(second.rule_set_subscriptions.is_empty());
    }
}
