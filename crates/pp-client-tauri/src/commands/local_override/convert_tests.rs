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
        builtin: false,
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
        builtins_seeded: true,
    }
}

/// A multi-tag target (with whitespace and duplicates) is accepted; dedup is not an error.
#[test]
fn accepts_multi_tag_and_duplicates() {
    let ovr = override_with(
        vec![custom_set("c1", "a"), custom_set("c2", "b")],
        vec![rule_set_rule("r1", "a, b,a")],
    );
    assert!(validate_local_override(&ovr).is_ok());
}

/// 引用不存在的规则集 tag **不再**拦截保存：规则集可被用户删除（或尚未下载），
/// 悬空引用由注入层降级剥离，核心仍可启动。
#[test]
fn accepts_unknown_tag_in_multi_tag_target() {
    let ovr = override_with(
        vec![custom_set("c1", "a")],
        vec![rule_set_rule("r1", "a, missing")],
    );
    assert!(validate_local_override(&ovr).is_ok());
}

/// An empty segment (`a,,b`) is invalid input.
#[test]
fn rejects_empty_segment() {
    let ovr = override_with(
        vec![custom_set("c1", "a"), custom_set("c2", "b")],
        vec![rule_set_rule("r1", "a,,b")],
    );
    let err = validate_local_override(&ovr).unwrap_err();
    assert!(err.contains("empty rule set tag"), "{err}");
}

/// An all-empty target (`,`) is invalid too.
#[test]
fn rejects_all_empty_segments() {
    let ovr = override_with(vec![], vec![rule_set_rule("r1", ",")]);
    assert!(validate_local_override(&ovr).is_err());
}

/// Legacy single-value targets keep validating as before.
#[test]
fn accepts_legacy_single_tag() {
    let ovr = override_with(vec![custom_set("c1", "a")], vec![rule_set_rule("r1", "a")]);
    assert!(validate_local_override(&ovr).is_ok());
}
