//! Store tests (split out of `store.rs` to stay within the
//! business-file size gate; see `.agents/rules/code-organization.md`).
//!
//! Covers `LocalOverrideStore` read/write resilience plus the idempotent
//! legacy migration run inside `load`: built-in subscription → custom rule
//! sets, cleanup of the removed scenario-template fields, and folding of the
//! removed `singbox.enabled` master switch into per-item switches.
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
}

#[test]
fn store_load_corrupted_file_falls_back() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    std::fs::write(store.override_file(), "not valid json {{{").unwrap();
    let ovr = store.load().unwrap();
    // Should fall back to default, not panic/error.
    assert!(ovr.singbox.rules.is_empty());
}

#[test]
fn store_save_and_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let mut ovr = LocalOverride::default();
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

    // 场景模板字段随功能移除一次性清空（写回空数组）。
    assert!(ovr.applied_templates.is_empty());
    assert!(ovr.custom_templates.is_empty());

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
    // 场景模板字段清空。
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
// Scenario-template removal cleanup
// -----------------------------------------------------------------------

/// 旧版文件携带模板字段（字符串引用与快照对象两种历史形态）→ load 清空并写回
/// 空数组；规则卡片（`singbox.rules`）不受影响。
#[test]
fn migration_clears_removed_scenario_template_fields() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
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
            "applied_templates": [
                { "template_id": "custom:tpl-1", "applied_at": 2, "generated_rule_ids": [] }
            ],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-1", "name": "旧模板", "desc": "",
                // 快照对象数组（更早的历史形态）也必须能解析后清空。
                "rules": [ { "id": "r1", "name": "a" } ],
                "created_at": 1
            }]
        }),
    );

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();

    assert!(ovr.applied_templates.is_empty());
    assert!(ovr.custom_templates.is_empty());
    // 规则卡片不被清理触碰。
    assert_eq!(ovr.singbox.rules.len(), 1);
    assert_eq!(ovr.singbox.rules[0].id, "r1");

    // 清理结果已写回磁盘：文件里模板段为空数组，再次 load 幂等。
    let reloaded = store.load().unwrap();
    assert!(reloaded.applied_templates.is_empty());
    assert!(reloaded.custom_templates.is_empty());
}

// -----------------------------------------------------------------------
// Rule master-switch removal migration
// -----------------------------------------------------------------------

/// 旧文件 `singbox.enabled == false` → 所有规则卡片与规则集引用置 `enabled=false`，
/// 并写回（字段不再序列化）；二次 load 幂等。
#[test]
fn migration_folds_disabled_master_switch_into_per_item_switches() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": false,
                "rules": [
                    { "id": "r1", "name": "a", "enabled": true,
                      "match_type": "domain_suffix", "target": "a.com",
                      "action": "proxy", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "r2", "name": "b", "enabled": true,
                      "match_type": "domain", "target": "b.com",
                      "action": "direct", "note": "", "created_at": 2, "sort_order": 1 }
                ],
                "rule_sets": [
                    { "id": "rs1", "name": "RS", "tag": "rs-tag",
                      "kind": "sing_box_remote",
                      "source": { "remote": { "url": "https://e/x.srs" } },
                      "enabled": true, "auto_update_interval_minutes": 0, "last_updated": 0 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": []
        }),
    );

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert_eq!(ovr.singbox.rules.len(), 2);
    assert!(ovr.singbox.rules.iter().all(|r| !r.enabled));
    assert_eq!(ovr.singbox.rule_sets.len(), 1);
    assert!(!ovr.singbox.rule_sets[0].enabled);

    // 写回后不再带 `singbox.enabled` 字段；二次 load 结果稳定（幂等）。
    let text = std::fs::read_to_string(store.override_file()).unwrap();
    let written: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert!(
        written["singbox"].get("enabled").is_none(),
        "master switch must not be serialized: {text}"
    );
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded, ovr);
}

/// 旧文件 `singbox.enabled == true`（或字段缺失）→ 逐项开关保持不变。
#[test]
fn migration_keeps_per_item_switches_when_master_enabled() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rules": [
                    { "id": "r1", "name": "a", "enabled": true,
                      "match_type": "domain_suffix", "target": "a.com",
                      "action": "proxy", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "r2", "name": "b", "enabled": false,
                      "match_type": "domain", "target": "b.com",
                      "action": "direct", "note": "", "created_at": 2, "sort_order": 1 }
                ],
                "rule_sets": []
            },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": []
        }),
    );

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert!(ovr.singbox.rules[0].enabled);
    assert!(!ovr.singbox.rules[1].enabled);
}
