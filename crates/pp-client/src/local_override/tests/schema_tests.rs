//! Local Override schema tests (split out of `schema.rs` to stay within the
//! business-file size gate; see `.agents/rules/code-organization.md`).

use super::*;

#[test]
fn local_override_serde_roundtrip() {
    let orig = LocalOverride {
        singbox: CoreLocalOverride {
            rules: vec![LocalRule {
                id: "r1".to_string(),
                name: "test rule".to_string(),
                enabled: true,
                match_type: RuleMatchType::DomainSuffix,
                target: "googleapis.com".to_string(),
                action: RuleAction::Proxy,
                advanced: RuleAdvancedOptions {
                    no_resolve: true,
                    invert: false,
                    _sniff: false,
                },
                note: "note".to_string(),
                created_at: 1234567890,
                sort_order: 0,
            }],
            rule_sets: vec![LocalRuleSetRef {
                id: "rs1".to_string(),
                name: "GeoIP CN".to_string(),
                tag: "geoip-cn".to_string(),
                kind: RuleSetKind::SingBoxRemote,
                source: RuleSetSource::Remote {
                    url: "https://example.com/cn.srs".to_string(),
                },
                enabled: true,
                auto_update_interval_minutes: 1440,
                last_updated: 1234567890,
            }],
            enabled: true,
        },
        rule_set_subscriptions: vec![RuleSetSubscription {
            id: "sub1".to_string(),
            community_id: "geoip-cn".to_string(),
            display_name: "GeoIP China".to_string(),
            category: RuleSetCategory::Geoip,
            subscribed: true,
            singbox_url_template:
                "https://github.com/MetaCubeX/meta-rules-dat/raw/sing/geo-lite/ip/{tag}.srs"
                    .to_string(),
            default_interval_minutes: 1440,
        }],
        applied_templates: vec![AppliedTemplate {
            template_id: "return-china".to_string(),
            applied_at: 1234567890,
            generated_rule_ids: vec!["r1".to_string()],
        }],
        custom_rule_sets: vec![
            CustomRuleSet {
                id: "c1".to_string(),
                name: "Custom Ads".to_string(),
                tag: "custom-ads".to_string(),
                source: CustomRuleSetSource::Remote {
                    url: "https://example.com/custom-ads.srs".to_string(),
                    format: RuleSetFormat::Binary,
                },
                last_updated: 1234567890,
            },
            CustomRuleSet {
                id: "c2".to_string(),
                name: "Manual list".to_string(),
                tag: "manual-list".to_string(),
                source: CustomRuleSetSource::Manual {
                    content: r#"[{"rules":[{"domain_suffix":"example.com"}]}]"#.to_string(),
                },
                last_updated: 0,
            },
        ],
        custom_templates: vec![CustomTemplate {
            id: "tpl1".to_string(),
            name: "我的场景".to_string(),
            desc: "规则引用组合".to_string(),
            rules: vec!["r1".to_string(), "r2".to_string()],
            created_at: 1234567890,
        }],
        market_sources: vec![MarketSource {
            id: "src1".to_string(),
            name: "示例市场".to_string(),
            url: "https://example.com/market.json".to_string(),
            last_fetched: 1234567890,
        }],
    };

    let json = serde_json::to_string(&orig).unwrap();
    let back: LocalOverride = serde_json::from_str(&json).unwrap();
    assert_eq!(orig, back);

    // Internal-tag representation for the enum is stable.
    assert!(json.contains("\"kind\":\"remote\""));
    assert!(json.contains("\"format\":\"binary\""));
    assert!(json.contains("\"kind\":\"manual\""));
}

#[test]
fn custom_rule_set_source_serde_shapes() {
    // Remote serializes with kind/url/format.
    let remote = CustomRuleSetSource::Remote {
        url: "https://e/x.srs".to_string(),
        format: RuleSetFormat::Binary,
    };
    let v: serde_json::Value = serde_json::to_value(&remote).unwrap();
    assert_eq!(v["kind"], "remote");
    assert_eq!(v["url"], "https://e/x.srs");
    assert_eq!(v["format"], "binary");

    // Manual serializes with kind/content.
    let manual = CustomRuleSetSource::Manual {
        content: "[]".to_string(),
    };
    let v: serde_json::Value = serde_json::to_value(&manual).unwrap();
    assert_eq!(v["kind"], "manual");
    assert_eq!(v["content"], "[]");

    // Both roundtrip.
    for src in [remote, manual] {
        let json = serde_json::to_string(&src).unwrap();
        let back: CustomRuleSetSource = serde_json::from_str(&json).unwrap();
        assert_eq!(src, back);
    }
}

#[test]
fn rule_set_format_serde_aligns_with_singbox_values() {
    assert_eq!(
        serde_json::to_string(&RuleSetFormat::Source).unwrap(),
        "\"source\""
    );
    assert_eq!(
        serde_json::to_string(&RuleSetFormat::Binary).unwrap(),
        "\"binary\""
    );
    assert_eq!(
        serde_json::from_str::<RuleSetFormat>("\"source\"").unwrap(),
        RuleSetFormat::Source
    );
    assert_eq!(
        serde_json::from_str::<RuleSetFormat>("\"binary\"").unwrap(),
        RuleSetFormat::Binary
    );
}

#[test]
fn serde_missing_fields_defaults() {
    // Old local_override.json missing new fields should deserialize with defaults.
    let json = r#"{
            "singbox": {
                "rules": []
            }
        }"#;
    let parsed: LocalOverride = serde_json::from_str(json).unwrap();
    assert!(parsed.rule_set_subscriptions.is_empty());
    assert!(parsed.applied_templates.is_empty());
    assert!(parsed.custom_rule_sets.is_empty());
    assert!(parsed.custom_templates.is_empty());
    assert!(parsed.market_sources.is_empty());
    assert!(parsed.singbox.enabled); // default_true
}

/// 旧版文件（无 `custom_templates` 段）反序列化为空 Vec。
#[test]
fn custom_templates_default_empty_for_legacy_file() {
    let json = r#"{
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": []
        }"#;
    let parsed: LocalOverride = serde_json::from_str(json).unwrap();
    assert!(parsed.custom_templates.is_empty());

    // 显式写入后 roundtrip 完整（规则 ID 引用列表）。
    let mut ovr = parsed;
    ovr.custom_templates.push(CustomTemplate {
        id: "tpl1".to_string(),
        name: "我的场景".to_string(),
        desc: String::new(),
        rules: vec!["r1".to_string(), "r2".to_string()],
        created_at: 100,
    });
    let saved = serde_json::to_string(&ovr).unwrap();
    let back: LocalOverride = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.custom_templates.len(), 1);
    assert_eq!(back.custom_templates[0].id, "tpl1");
    assert_eq!(
        back.custom_templates[0].rules,
        vec!["r1".to_string(), "r2".to_string()]
    );
}

/// 旧版文件（无 `custom_rule_sets` 段）反序列化为空 Vec。
#[test]
fn custom_rule_sets_default_empty_for_legacy_file() {
    let json = r#"{
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": []
        }"#;
    let parsed: LocalOverride = serde_json::from_str(json).unwrap();
    assert!(parsed.custom_rule_sets.is_empty());

    // 显式写入后 roundtrip 完整。
    let mut ovr = parsed;
    ovr.custom_rule_sets.push(CustomRuleSet {
        id: "c1".to_string(),
        name: String::new(),
        tag: "my-set".to_string(),
        source: CustomRuleSetSource::Remote {
            url: "https://e/x.srs".to_string(),
            format: RuleSetFormat::Binary,
        },
        last_updated: 0,
    });
    let saved = serde_json::to_string(&ovr).unwrap();
    let back: LocalOverride = serde_json::from_str(&saved).unwrap();
    assert_eq!(back.custom_rule_sets.len(), 1);
    assert_eq!(back.custom_rule_sets[0].tag, "my-set");
}

/// 纯资源语义兼容：旧文件里 `custom_rule_sets[].enabled` 字段被 serde 静默忽略。
#[test]
fn custom_rule_set_legacy_enabled_field_is_ignored() {
    let json = r#"{
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [{
                "id": "c1", "name": "x", "tag": "t",
                "source": { "kind": "remote", "url": "https://e/x.srs", "format": "binary" },
                "enabled": false,
                "last_updated": 0
            }],
            "custom_templates": []
        }"#;
    let parsed: LocalOverride = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.custom_rule_sets.len(), 1);
    assert_eq!(parsed.custom_rule_sets[0].tag, "t");
    // 规则集条目序列化回写不再带 enabled 字段（纯资源语义）。
    let entry = serde_json::to_string(&parsed.custom_rule_sets[0]).unwrap();
    assert!(!entry.contains("\"enabled\""));
}

/// 纯资源语义：模板是纯规则 ID 引用列表，反序列化只接受字符串数组。
#[test]
fn custom_template_rules_are_string_references() {
    let json = r#"{
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl1", "name": "t", "desc": "",
                "rules": ["r1", "r2"],
                "created_at": 1
            }]
        }"#;
    let parsed: LocalOverride = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.custom_templates[0].rules, vec!["r1", "r2"]);
}

#[test]
fn custom_rule_set_file_format_matches_source() {
    let remote_bin = CustomRuleSet {
        id: "c1".to_string(),
        name: String::new(),
        tag: "t".to_string(),
        source: CustomRuleSetSource::Remote {
            url: "https://e/x.srs".to_string(),
            format: RuleSetFormat::Binary,
        },
        last_updated: 0,
    };
    assert_eq!(remote_bin.file_format(), RuleSetFormat::Binary);

    let remote_src = CustomRuleSet {
        source: CustomRuleSetSource::Remote {
            url: "https://e/x.json".to_string(),
            format: RuleSetFormat::Source,
        },
        ..remote_bin.clone()
    };
    assert_eq!(remote_src.file_format(), RuleSetFormat::Source);

    let manual = CustomRuleSet {
        source: CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
        ..remote_bin
    };
    assert_eq!(manual.file_format(), RuleSetFormat::Source);
}

/// 存量兼容：旧版 `local_override.json` 的 `mihomo` 桶与 `mihomo_url_template` 字段
/// 被静默丢弃，不影响解析。
#[test]
fn serde_tolerates_legacy_mihomo_fields() {
    let json = r#"{
            "singbox": { "rules": [] },
            "mihomo": { "rules": [{"id":"old","match_type":"domain","target":"x.com","action":"direct","created_at":1,"sort_order":0}] },
            "rule_set_subscriptions": [{
                "id": "sub1",
                "community_id": "geoip-cn",
                "display_name": "GeoIP China",
                "category": "geoip",
                "subscribed": true,
                "singbox_url_template": "https://example.com/{tag}.srs",
                "mihomo_url_template": "https://example.com/{tag}.yaml",
                "default_interval_minutes": 1440
            }]
        }"#;
    let parsed: LocalOverride = serde_json::from_str(json).unwrap();
    assert!(parsed.singbox.rules.is_empty());
    assert_eq!(parsed.rule_set_subscriptions.len(), 1);
}

#[test]
fn rule_action_outbound_tag() {
    assert_eq!(RuleAction::Proxy.outbound_tag(), "proxy");
    assert_eq!(RuleAction::Direct.outbound_tag(), "direct");
    assert_eq!(RuleAction::Reject.outbound_tag(), "reject");
    assert_eq!(
        RuleAction::Outbound {
            tag: "custom".to_string()
        }
        .outbound_tag(),
        "custom"
    );
}
