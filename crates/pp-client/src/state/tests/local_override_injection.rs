//! Local override 注入回归测试（引用 + 应用激活语义）。
//!
//! - 注入的规则 = `singbox.rules` 中 `enabled && id ∈ 激活集合` 的规则；激活集合
//!   来自已应用模板的引用列表（[`active_rule_ids`]）。
//! - `route.rule_set` 条目只来自被注入规则引用的 custom rule sets（纯资源、无
//!   enabled；backing 文件须存在）。

use crate::local_override::{LocalOverrideStore, RuleSetFormat, RuleSetManager, apply_template};

use crate::local_override::inject_local_override_warn_only;

/// 构造一份旧版文件：custom Remote 规则集 + 一条 `rule_set` 规则卡片 + 已应用的
/// custom 模板（引用该规则）。经 load 后（snapshot→引用迁移幂等）注入只走引用。
fn write_legacy_file(dir: &tempfile::TempDir, json: serde_json::Value) {
    std::fs::write(
        dir.path().join("local_override.json"),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
}

#[test]
fn injection_only_injects_applied_template_rules() {
    let dir = tempfile::tempdir().unwrap();
    // 两个 live 规则：r1（被已应用模板引用）、r2（不在任何已应用模板里）。
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rule_sets": [],
                "rules": [
                    { "id": "r1", "name": "geoip direct", "enabled": true,
                      "match_type": "rule_set", "target": "geoip-cn",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "r2", "name": "ads reject", "enabled": true,
                      "match_type": "domain_suffix", "target": "ads.example",
                      "action": "reject", "note": "", "created_at": 1, "sort_order": 1 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [
                { "template_id": "custom:tpl-1", "applied_at": 1, "generated_rule_ids": [] }
            ],
            "custom_rule_sets": [{
                "id": "geoip-cn", "name": "GeoIP CN", "tag": "geoip-cn",
                "source": { "kind": "remote",
                            "url": "https://example.com/ip/geoip-cn.srs",
                            "format": "binary" },
                "last_updated": 0
            }],
            "custom_templates": [{
                "id": "tpl-1", "name": "直连国内", "desc": "",
                // snapshot 对象数组 → 迁移为引用 ["r1"]
                "rules": [ { "id": "r1", "name": "geoip direct", "enabled": true,
                             "match_type": "rule_set", "target": "geoip-cn",
                             "action": "direct", "note": "", "created_at": 1, "sort_order": 0 } ],
                "created_at": 1
            }]
        }),
    );

    // 触发迁移（load 幂等归一化并把结果写回磁盘）。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert!(ovr.rule_set_subscriptions.is_empty());
    assert_eq!(ovr.applied_templates.len(), 1);
    assert_eq!(ovr.singbox.rules.len(), 2);
    // snapshot → 引用迁移。
    assert_eq!(ovr.custom_templates[0].rules, vec!["r1"]);

    // 伪造 custom Remote 的 backing 文件，使其可被注入。
    let manager = RuleSetManager::new(dir.path().to_path_buf());
    let file = manager.custom_rule_set_file_path("geoip-cn", RuleSetFormat::Binary);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "fake-srs").unwrap();

    // 注入进生成配置。
    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    // 只注入已应用模板引用的启用规则（r1），未引用的 r2 不注入。
    let rules = config["route"]["rules"].as_array().unwrap();
    assert!(
        rules.iter().any(|r| r["rule_set"] == "geoip-cn"),
        "r1 (referenced by applied template) must be injected"
    );
    assert!(
        !rules.iter().any(|r| r["domain_suffix"] == "ads.example"),
        "r2 (not referenced by any applied template) must NOT be injected"
    );

    // rule_set 条目 = 被注入规则引用且已落盘 backing 的 custom set。
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

#[test]
fn injection_skips_disabled_and_dangling_refs_but_injects_valid_ones() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rule_sets": [],
                "rules": [
                    { "id": "r-ok", "name": "ok", "enabled": true,
                      "match_type": "domain", "target": "ok.example",
                      "action": "proxy", "note": "", "created_at": 1, "sort_order": 0 },
                    { "id": "r-off", "name": "off", "enabled": false,
                      "match_type": "domain", "target": "off.example",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 1 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [
                { "template_id": "custom:tpl-multi", "applied_at": 1, "generated_rule_ids": [] }
            ],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-multi", "name": "混合", "desc": "",
                // 引用 r-ok（启用）、r-off（disabled → 失效）、r-ghost（不存在 → 失效）
                "rules": ["r-ok", "r-off", "r-ghost"],
                "created_at": 1
            }]
        }),
    );

    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let ovr = store.load().unwrap();
    assert_eq!(
        ovr.custom_templates[0].rules,
        vec!["r-ok", "r-off", "r-ghost"]
    );

    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    let rules = config["route"]["rules"].as_array().unwrap();
    // Valid referenced rule injected.
    assert!(rules.iter().any(|r| r["domain"] == "ok.example"));
    // Disabled referenced rule not injected.
    assert!(!rules.iter().any(|r| r["domain"] == "off.example"));
    // Missing rule simply has nothing to inject.
    assert_eq!(rules.len(), 1, "{rules:?}");
}

#[test]
fn injection_no_applied_template_injects_nothing_even_with_rules() {
    let dir = tempfile::tempdir().unwrap();
    write_legacy_file(
        &dir,
        serde_json::json!({
            "singbox": {
                "enabled": true,
                "rule_sets": [],
                "rules": [
                    { "id": "r1", "name": "a", "enabled": true,
                      "match_type": "domain", "target": "a.example",
                      "action": "direct", "note": "", "created_at": 1, "sort_order": 0 }
                ]
            },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl-1", "name": "t", "desc": "", "rules": ["r1"], "created_at": 1
            }]
        }),
    );

    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    // 模板存在但未应用 → 场景未激活，不注入任何本地规则。
    let rules = config["route"]["rules"].as_array().unwrap();
    assert!(rules.is_empty(), "{rules:?}");
}

#[test]
fn injection_multi_applied_templates_union() {
    let dir = tempfile::tempdir().unwrap();
    // 通过 pp-client 构造状态再落盘，走一遍 apply 链路。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let mut ovr = crate::local_override::LocalOverride::default();
    ovr.singbox.rules = vec![
        crate::local_override::LocalRule {
            id: "r1".to_string(),
            name: "a".to_string(),
            enabled: true,
            match_type: crate::local_override::RuleMatchType::Domain,
            target: "a.example".to_string(),
            action: crate::local_override::RuleAction::Proxy,
            advanced: Default::default(),
            note: String::new(),
            created_at: 1,
            sort_order: 0,
        },
        crate::local_override::LocalRule {
            id: "r2".to_string(),
            name: "b".to_string(),
            enabled: true,
            match_type: crate::local_override::RuleMatchType::Domain,
            target: "b.example".to_string(),
            action: crate::local_override::RuleAction::Direct,
            advanced: Default::default(),
            note: String::new(),
            created_at: 1,
            sort_order: 1,
        },
    ];
    ovr.custom_templates = vec![
        crate::local_override::CustomTemplate {
            id: "t1".to_string(),
            name: "一".to_string(),
            desc: String::new(),
            rules: vec!["r1".to_string()],
            created_at: 1,
        },
        crate::local_override::CustomTemplate {
            id: "t2".to_string(),
            name: "二".to_string(),
            desc: String::new(),
            rules: vec!["r2".to_string()],
            created_at: 1,
        },
    ];
    apply_template(&mut ovr, "custom:t1", 2).unwrap();
    apply_template(&mut ovr, "custom:t2", 3).unwrap();
    store.save(&ovr).unwrap();

    let mut config = serde_json::json!({ "route": { "rules": [], "final": "direct" } });
    inject_local_override_warn_only(dir.path(), &mut config);

    // 多模板叠加 = 并集：r1 与 r2 均注入。
    let rules = config["route"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 2, "{rules:?}");
    assert!(rules.iter().any(|r| r["domain"] == "a.example"));
    assert!(rules.iter().any(|r| r["domain"] == "b.example"));
}
