//! Local override 注入回归测试（全量启用规则语义）。
//!
//! 场景模板已移除：注入的规则 = `singbox.rules` 中 **`enabled` 的全部规则**，
//! 不再按模板引用过滤。旧文件中的模板字段仅作 serde 兼容，load 时清空，对注入
//! 无影响。
//!
//! - `route.rule_set` 条目只来自被注入规则引用的 custom rule sets（纯资源、无
//!   enabled；backing 文件须存在）。

use crate::local_override::inject_local_override_warn_only;
use crate::local_override::{LocalOverrideStore, RuleSetFormat, RuleSetManager};

/// 写入一份（可能带旧模板字段的）`local_override.json`。
fn write_legacy_file(dir: &tempfile::TempDir, json: serde_json::Value) {
    std::fs::write(
        dir.path().join("local_override.json"),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
}

/// 未引用任何模板的 enabled 规则也会注入；disabled 规则不注入。
///
/// 旧文件里同时保留 applied/custom 模板字段，用于证明模板不再参与过滤。
#[test]
fn injection_injects_all_enabled_rules_regardless_of_templates() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rule_sets": [],
                "rules": [
                    { "id": "r1", "name": "ads reject", "enabled": true,
                      "match_type": "domain_suffix", "target": "ads.example",
                      "action": "reject", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "r2", "name": "off", "enabled": false,
                      "match_type": "domain", "target": "off.example",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 1 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [
                { "template_id": "custom:tpl-1", "applied_at": 1, "generated_rule_ids": [] }
            ],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-1", "name": "直连国内", "desc": "",
                "rules": [], "created_at": 1
            }]
        }),
    );

    // load 清空模板字段，规则卡片保持。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert!(ovr.custom_templates.is_empty());
    assert!(ovr.applied_templates.is_empty());
    assert_eq!(ovr.singbox.rules.len(), 3, "2 用户规则 + 1 播种内置规则");

    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    let rules = config["route"]["rules"].as_array().unwrap();
    assert!(
        rules.iter().any(|r| r["domain_suffix"] == "ads.example"),
        "enabled rule must be injected regardless of template references: {rules:?}"
    );
    assert!(
        !rules.iter().any(|r| r["domain"] == "off.example"),
        "disabled rule must NOT be injected: {rules:?}"
    );
    // 1 条启用的用户规则 + 1 条启用的播种内置规则。
    assert_eq!(rules.len(), 2, "{rules:?}");
}

/// 无模板字段的普通文件：所有 enabled 规则注入。
#[test]
fn injection_injects_enabled_rules_without_any_template_data() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rule_sets": [],
                "rules": [
                    { "id": "a", "name": "a", "enabled": true,
                      "match_type": "domain", "target": "a.example",
                      "action": "proxy", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "b", "name": "b", "enabled": true,
                      "match_type": "domain", "target": "b.example",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 1 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": []
        }),
    );

    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    let rules = config["route"]["rules"].as_array().unwrap();
    // 2 条用户规则 + 1 条播种内置规则（统一排序列表，内置置顶）。
    assert_eq!(rules.len(), 3, "{rules:?}");
    assert!(rules.iter().any(|r| r["domain"] == "a.example"));
    assert!(rules.iter().any(|r| r["domain"] == "b.example"));
    assert!(
        rules
            .iter()
            .any(|r| r["rule_set"] == serde_json::json!(["geosite-private", "geoip-private"])),
        "seeded builtin private-direct rule injected: {rules:?}"
    );
}

/// `route.rule_set` 条目 = 被注入规则引用且已落盘 backing 的 custom set；
/// 未被引用的 custom set 不注入。
#[test]
fn injection_rule_set_entries_only_for_referenced_custom_sets() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rule_sets": [],
                "rules": [
                    { "id": "r1", "name": "geoip direct", "enabled": true,
                      "match_type": "rule_set", "target": "geoip-cn",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 0 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [
                { "id": "geoip-cn", "name": "GeoIP CN", "tag": "geoip-cn",
                  "source": { "kind": "remote",
                              "url": "https://example.com/ip/geoip-cn.srs",
                              "format": "binary" },
                  "last_updated": 0 },
                { "id": "unused", "name": "Unused", "tag": "unused",
                  "source": { "kind": "remote",
                              "url": "https://example.com/unused.srs",
                              "format": "binary" },
                  "last_updated": 0 }
            ],
            "custom_templates": []
        }),
    );

    // 伪造 geoip-cn 的 backing 文件，使其可被注入。
    let manager = RuleSetManager::new(dir.path().to_path_buf());
    let file = manager.custom_rule_set_file_path("geoip-cn", RuleSetFormat::Binary);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "fake-srs").unwrap();

    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    let rules = config["route"]["rules"].as_array().unwrap();
    assert!(
        rules.iter().any(|r| r["rule_set"] == "geoip-cn"),
        "rule_set rule must be injected: {rules:?}"
    );

    assert!(
        config["route"].get("rule_sets").is_none(),
        "no plural rule_sets key allowed"
    );
    let rule_set = config["route"]["rule_set"].as_array().unwrap();
    assert_eq!(rule_set.len(), 1, "{rule_set:?}");
    assert_eq!(rule_set[0]["type"], "local");
    assert_eq!(rule_set[0]["tag"], "geoip-cn");
    assert_eq!(rule_set[0]["format"], "binary");
}
