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
        applied_templates: Vec::new(),
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
                remote_updated_at: 0,
            },
            CustomRuleSet {
                id: "c2".to_string(),
                name: "Manual list".to_string(),
                tag: "manual-list".to_string(),
                source: CustomRuleSetSource::Manual {
                    content: r#"[{"rules":[{"domain_suffix":"example.com"}]}]"#.to_string(),
                },
                last_updated: 0,
                remote_updated_at: 0,
            },
        ],
        custom_templates: Vec::new(),
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
    assert!(parsed.applied_templates.is_empty());
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
        remote_updated_at: 0,
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

/// 场景模板已移除：`custom_templates` 仅作 serde 兼容字段，旧文件中的
/// 字符串引用数组与更早的快照对象数组都必须能无损解析（不因形态而失败）。
#[test]
fn legacy_custom_template_shapes_parse_as_opaque_compat_values() {
    for rules in [
        serde_json::json!(["r1", "r2"]),
        serde_json::json!([{ "id": "r1", "name": "a" }, { "id": "r2" }]),
    ] {
        let json = serde_json::json!({
            "singbox": { "rules": [], "rule_sets": [], "enabled": true },
            "rule_set_subscriptions": [],
            "applied_templates": [],
            "custom_rule_sets": [],
            "custom_templates": [{
                "id": "tpl1", "name": "t", "desc": "",
                "rules": rules,
                "created_at": 1
            }]
        });
        let parsed: LocalOverride = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.custom_templates.len(), 1);
    }
}

/// `applied_templates` 同为 serde 兼容字段：任意历史形态都能解析。
#[test]
fn legacy_applied_template_records_parse_as_opaque_compat_values() {
    let json = serde_json::json!({
        "singbox": { "rules": [], "rule_sets": [], "enabled": true },
        "rule_set_subscriptions": [],
        "applied_templates": [
            { "template_id": "return-china", "applied_at": 1, "generated_rule_ids": ["r1"] },
            { "template_id": "custom:tpl", "applied_at": 2 }
        ],
        "custom_rule_sets": [],
        "custom_templates": []
    });
    let parsed: LocalOverride = serde_json::from_value(json).unwrap();
    assert_eq!(parsed.applied_templates.len(), 2);
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
        remote_updated_at: 0,
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

// -----------------------------------------------------------------------
// parse_rule_set_tags
// -----------------------------------------------------------------------

/// 存量单值数据天然兼容：单 tag 解析为一元素 Vec。
#[test]
fn parse_rule_set_tags_single_value_legacy_compatible() {
    assert_eq!(
        parse_rule_set_tags("geosite-cn"),
        vec!["geosite-cn".to_string()]
    );
}

/// 多 tag：按逗号拆分、逐段 trim、去重（保留首次出现顺序）。
#[test]
fn parse_rule_set_tags_splits_trims_and_dedups() {
    assert_eq!(
        parse_rule_set_tags(" geosite-cn , my-custom,geosite-cn "),
        vec!["geosite-cn".to_string(), "my-custom".to_string()]
    );
}

/// 空段被丢弃；全空段/空串得到空 Vec。
#[test]
fn parse_rule_set_tags_drops_empty_segments() {
    assert_eq!(
        parse_rule_set_tags("a,,  ,b,"),
        vec!["a".to_string(), "b".to_string()]
    );
    assert!(parse_rule_set_tags(" , ").is_empty());
    assert!(parse_rule_set_tags("").is_empty());
}
