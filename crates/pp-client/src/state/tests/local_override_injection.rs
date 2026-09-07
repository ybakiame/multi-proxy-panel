//! Local override 注入回归测试：生成配置的 `rule_sets` 只来自用户自控的
//! custom rule sets（内置订阅的 remote rule_set 注入已移除）。

use crate::local_override::{LocalOverrideStore, RuleSetFormat, RuleSetManager};

use crate::state::inject_local_override_warn_only;

/// 存量迁移 + 注入：旧文件里「已订阅内置 + 内置模板生成的 rule_set 规则卡片」经
/// `load` 迁移为 custom Remote 后，注入进生成配置的 rule_sets 唯一来源是 custom
/// local 条目；不存在任何内置 remote rule_set 条目。
#[test]
fn injection_rule_sets_only_come_from_custom_after_legacy_migration() {
    let dir = tempfile::tempdir().unwrap();
    // 旧版文件：geoip-cn 已订阅（内置 remote 模板）+ return-china 模板生成了一条
    // rule_set 规则卡片（指向 geoip-cn）。
    let legacy = serde_json::json!({
        "singbox": {
            "enabled": true,
            "rule_sets": [],
            "rules": [{
                "id": "gen-1", "name": "GeoIP CN direct", "enabled": true,
                "match_type": "rule_set", "target": "geoip-cn",
                "action": "direct", "note": "", "created_at": 1, "sort_order": -100
            }]
        },
        "rule_set_subscriptions": [{
            "id": "sub-geoip-cn",
            "community_id": "geoip-cn",
            "display_name": "GeoIP China",
            "category": "geoip",
            "subscribed": true,
            "singbox_url_template": "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/ip/{tag}.srs",
            "default_interval_minutes": 1440
        }],
        "applied_templates": [
            { "template_id": "return-china", "applied_at": 1, "generated_rule_ids": ["gen-1"] }
        ],
        "custom_rule_sets": [],
        "custom_templates": []
    });
    let path = dir.path().join("local_override.json");
    std::fs::write(&path, serde_json::to_string_pretty(&legacy).unwrap()).unwrap();

    // 触发迁移（load 幂等归一化并把结果写回磁盘）。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert!(ovr.rule_set_subscriptions.is_empty());
    assert!(ovr.applied_templates.is_empty());
    assert_eq!(ovr.singbox.rules.len(), 1, "内置模板生成的规则保留");
    assert_eq!(ovr.custom_rule_sets.len(), 1);

    // 伪造迁移出的 custom Remote 的 backing 文件，使其可被注入。
    let migrated = &ovr.custom_rule_sets[0];
    assert_eq!(migrated.tag, "geoip-cn");
    let manager = RuleSetManager::new(dir.path().to_path_buf());
    let file = manager.custom_rule_set_file_path(&migrated.id, RuleSetFormat::Binary);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "fake-srs").unwrap();

    // 注入进生成配置。
    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    // rule_set 规则卡片仍前插（引用 migrated tag）。
    let rules = config["route"]["rules"].as_array().unwrap();
    assert!(rules.iter().any(|r| r["rule_set"] == "geoip-cn"));

    // rule_sets 条目唯一来源为 custom local；无内置 remote 条目。
    let rule_sets = config["route"]["rule_sets"].as_array().unwrap();
    assert_eq!(rule_sets.len(), 1, "{rule_sets:?}");
    assert_eq!(rule_sets[0]["type"], "local");
    assert_eq!(rule_sets[0]["tag"], "geoip-cn");
    assert_eq!(rule_sets[0]["format"], "binary");
}
