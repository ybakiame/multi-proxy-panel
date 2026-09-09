//! Store tests (split out of `store.rs` to stay within the
//! business-file size gate; see `.agents/rules/code-organization.md`).
//!
//! Covers `LocalOverrideStore` read/write resilience plus the two idempotent
//! legacy migrations run inside `load`: built-in subscription → custom rule
//! sets, and template rule snapshots → rule-ID references.
//!

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

// -----------------------------------------------------------------------
// Snapshot → reference migration (reference-semantics refactor)
// -----------------------------------------------------------------------

/// 构造一份旧版文件：custom_templates 携带快照规则对象数组。
fn legacy_snapshot_template_file(dir: &tempfile::TempDir) {
    write_legacy_file(
        dir,
        serde_json::json!({
            "singbox": {
                "enabled": true, "rule_sets": [],
                "rules": [{
                    "id": "r1", "name": "a", "enabled": true,
                    "match_type": "domain_suffix", "target": "a.com",
                    "action": "proxy", "note": "", "created_at": 1, "sort_order": 0
                }]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-1", "name": "旧模板", "desc": "",
                "rules": [
                    { "id": "r1", "name": "a", "enabled": true,
                      "match_type": "domain_suffix", "target": "a.com",
                      "action": "proxy", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "r2", "name": "b", "enabled": true,
                      "match_type": "domain_suffix", "target": "b.com",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 1 }
                ],
                "created_at": 1
            }]
        }),
    );
}

#[test]
fn migration_converts_template_snapshot_to_id_refs() {
    let dir = tempfile::tempdir().unwrap();
    legacy_snapshot_template_file(&dir);

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();

    assert_eq!(ovr.custom_templates.len(), 1);
    let tpl = &ovr.custom_templates[0];
    assert_eq!(tpl.id, "tpl-1");
    // 快照规则数组 → 规则 ID 引用列表（保留原 id 顺序）。
    assert_eq!(tpl.rules, vec!["r1", "r2"]);
    // 规则列表（用户卡片）不被迁移触碰。
    assert_eq!(ovr.singbox.rules.len(), 1);
    assert_eq!(ovr.singbox.rules[0].id, "r1");

    // 迁移结果已写回磁盘：再次 load 幂等（rules 保持字符串引用）。
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded.custom_templates[0].rules, vec!["r1", "r2"]);
}

#[test]
fn migration_template_snapshot_refs_drop_objects_without_id() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-1", "name": "t", "desc": "",
                "rules": [ { "id": "r1", "name": "a" }, { "name": "no-id" } ],
                "created_at": 1
            }]
        }),
    );

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    // 无 id 的对象被跳过，可解析对象转引用。
    assert_eq!(ovr.custom_templates[0].rules, vec!["r1"]);
}

#[test]
fn migration_template_refs_already_strings_unchanged() {
    // 已是新格式（字符串引用）的文件无需迁移：load 不触发写盘改动。
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-1", "name": "t", "desc": "",
                "rules": ["r1", "r2"], "created_at": 1
            }]
        }),
    );

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert_eq!(ovr.custom_templates[0].rules, vec!["r1", "r2"]);
    // 二次读取一致（幂等）。
    let ovr2 = store.load().unwrap();
    assert_eq!(ovr2.custom_templates[0].rules, vec!["r1", "r2"]);
}
