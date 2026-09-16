//! `rule_set` multi-tag validation tests (split out of `convert.rs` to stay
//! under the business-file size warning threshold; see
//! `.agents/rules/code-organization.md`).

use super::*;
use pp_client::local_override::{CustomRuleSet, CustomRuleSetSource, RuleAction, RuleMatchType};

fn custom_set(id: &str, tag: &str) -> CustomRuleSet {
    CustomRuleSet {
        id: id.to_string(),
        name: String::new(),
        tag: tag.to_string(),
        source: CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
        last_updated: 0,
        remote_updated_at: 0,
    }
}

fn rule_set_rule(id: &str, target: &str) -> LocalRule {
    LocalRule {
        id: id.to_string(),
        name: String::new(),
        enabled: true,
        match_type: RuleMatchType::RuleSet,
        target: target.to_string(),
        action: RuleAction::Proxy,
        advanced: Default::default(),
        note: String::new(),
        created_at: 1,
        sort_order: 0,
        builtin: false,
    }
}
fn override_with(sets: Vec<CustomRuleSet>, rules: Vec<LocalRule>) -> LocalOverride {
    LocalOverride {
        singbox: CoreLocalOverride {
            rules,
            rule_sets: Vec::new(),
        },
        rule_set_subscriptions: Vec::new(),
        applied_templates: Vec::new(),
        custom_rule_sets: sets,
        custom_templates: Vec::new(),
    }
}

/// 多 tag（含空白、重复）均存在时通过；去重不报错。
#[test]
fn accepts_multi_tag_and_duplicates() {
    let ovr = override_with(
        vec![custom_set("c1", "a"), custom_set("c2", "b")],
        vec![rule_set_rule("r1", "a, b,a")],
    );
    assert!(validate_local_override(&ovr).is_ok());
}

/// 多 tag 中任一 tag 不存在即失败，错误信息包含缺失 tag。
#[test]
fn rejects_unknown_tag_in_multi_tag_target() {
    let ovr = override_with(
        vec![custom_set("c1", "a")],
        vec![rule_set_rule("r1", "a, missing")],
    );
    let err = validate_local_override(&ovr).unwrap_err();
    assert!(err.contains("missing"), "{err}");
}

/// 空段（`a,,b`）视为非法输入。
#[test]
fn rejects_empty_segment() {
    let ovr = override_with(
        vec![custom_set("c1", "a"), custom_set("c2", "b")],
        vec![rule_set_rule("r1", "a,,b")],
    );
    let err = validate_local_override(&ovr).unwrap_err();
    assert!(err.contains("empty rule set tag"), "{err}");
}

/// 全空段（`,`）同样非法。
#[test]
fn rejects_all_empty_segments() {
    let ovr = override_with(vec![], vec![rule_set_rule("r1", ",")]);
    assert!(validate_local_override(&ovr).is_err());
}

/// 单值存量数据仍按原行为校验通过。
#[test]
fn accepts_legacy_single_tag() {
    let ovr = override_with(vec![custom_set("c1", "a")], vec![rule_set_rule("r1", "a")]);
    assert!(validate_local_override(&ovr).is_ok());
}
