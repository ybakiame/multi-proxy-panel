//! Scenario template tests (split out of `template.rs` to stay within the
//! business-file size gate; see `.agents/rules/code-organization.md`).
//!
//! 自「场景模板改为规则引用 + 应用激活」起仅覆盖 custom-only 引用语义：apply 只
//! 记应用记录（不复制规则）、revert 只删记录（规则不动）、多模板叠加并集、失效
//! 引用（disabled/不存在）报告与注入跳过。

use super::*;
use crate::local_override::{
    LocalRule, RuleAction, RuleMatchType, RuleSetCategory, RuleSetSubscription,
};

fn empty_override() -> LocalOverride {
    LocalOverride::default()
}

/// Build a (live) rule card stored in `singbox.rules`.
fn live_rule(id: &str, enabled: bool, name: &str) -> LocalRule {
    LocalRule {
        id: id.to_string(),
        name: name.to_string(),
        enabled,
        match_type: RuleMatchType::DomainSuffix,
        target: format!("{id}.com"),
        action: RuleAction::Proxy,
        advanced: Default::default(),
        note: String::new(),
        created_at: 100,
        sort_order: 7,
    }
}

/// Template referencing the given live rule IDs.
fn ref_template(id: &str, name: &str, rule_ids: &[&str]) -> CustomTemplate {
    CustomTemplate {
        id: id.to_string(),
        name: name.to_string(),
        desc: "规则引用组合".to_string(),
        rules: rule_ids.iter().map(|s| s.to_string()).collect(),
        created_at: 500,
    }
}

/// Override with live rules + a template referencing them.
fn ovr_with(template: &CustomTemplate, live: &[LocalRule]) -> LocalOverride {
    let mut ovr = empty_override();
    ovr.singbox.rules.extend(live.iter().cloned());
    ovr.custom_templates.push(template.clone());
    ovr
}

// ---------------------------------------------------------------------------
// Apply semantics: record only, no rule copy
// ---------------------------------------------------------------------------

#[test]
fn apply_custom_template_records_applied_without_copying_rules() {
    let template = ref_template("tpl-1", "我的模板", &["r1", "r2"]);
    let mut ovr = ovr_with(
        &template,
        &[live_rule("r1", true, "a"), live_rule("r2", true, "b")],
    );
    let before = ovr.singbox.rules.len();

    let refs = apply_template(&mut ovr, "custom:tpl-1", 600).unwrap();

    // No rules copied / created / removed.
    assert_eq!(ovr.singbox.rules.len(), before);
    assert_eq!(ovr.singbox.rules[0].id, "r1");
    assert_eq!(ovr.singbox.rules[1].id, "r2");
    // Returns the template's reference list.
    assert_eq!(refs, template.rules);

    // Applied record uses "custom:<id>" with the deprecated generated list empty.
    assert_eq!(ovr.applied_templates.len(), 1);
    assert_eq!(ovr.applied_templates[0].template_id, "custom:tpl-1");
    assert_eq!(ovr.applied_templates[0].applied_at, 600);
    assert!(ovr.applied_templates[0].generated_rule_ids.is_empty());
}

#[test]
fn apply_custom_template_reapply_is_idempotent() {
    let template = ref_template("tpl-x", "模板", &["r1"]);
    let mut ovr = ovr_with(&template, &[live_rule("r1", true, "a")]);

    apply_template(&mut ovr, "custom:tpl-x", 600).unwrap();
    let second = apply_template(&mut ovr, "custom:tpl-x", 700).unwrap();

    // Single record, no duplicates; both calls return the reference list.
    assert_eq!(ovr.applied_templates.len(), 1);
    assert_eq!(second, template.rules);
}

#[test]
fn apply_template_rejects_builtin_or_unknown_ids() {
    let template = ref_template("tpl-1", "模板", &["r1"]);
    let mut ovr = ovr_with(&template, &[live_rule("r1", true, "a")]);

    // 旧内置模板 id 不再可用（机制废弃）。
    let err = apply_template(&mut ovr, "return-china", 1000)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown template id"), "{err}");
    // 无前缀 / 畸形前缀同样报错。
    assert!(apply_template(&mut ovr, "unknown", 1000).is_err());
    assert!(apply_template(&mut ovr, "custom", 1000).is_err());
    assert!(apply_template(&mut ovr, "custom:", 1000).is_err());
    // 报错不留半成品记录。
    assert!(ovr.applied_templates.is_empty());
    assert_eq!(ovr.singbox.rules.len(), 1);
}

#[test]
fn apply_unknown_custom_template_errors() {
    let mut ovr = empty_override();
    let result = apply_template(&mut ovr, "custom:does-not-exist", 1000);
    assert!(result.is_err());
    assert!(ovr.applied_templates.is_empty());
}

#[test]
fn apply_template_does_not_touch_rule_set_subscriptions() {
    // 引用语义下 apply 不订阅任何规则集。
    let template = ref_template("tpl-sub", "模板", &["r1"]);
    let mut ovr = ovr_with(&template, &[live_rule("r1", true, "a")]);
    ovr.rule_set_subscriptions.push(RuleSetSubscription {
        id: "sub-legacy".to_string(),
        community_id: "geoip-cn".to_string(),
        display_name: "Legacy".to_string(),
        category: RuleSetCategory::Geoip,
        subscribed: false,
        singbox_url_template: "https://example.com/{tag}.srs".to_string(),
        default_interval_minutes: 1440,
    });

    apply_template(&mut ovr, "custom:tpl-sub", 600).unwrap();

    assert!(ovr.rule_set_subscriptions.iter().all(|s| !s.subscribed));
}

// ---------------------------------------------------------------------------
// Revert semantics: record removal only, rules untouched
// ---------------------------------------------------------------------------

#[test]
fn revert_template_removes_record_but_keeps_rules() {
    let template = ref_template("tpl-revert", "模板", &["r1", "r2"]);
    let mut ovr = ovr_with(
        &template,
        &[live_rule("r1", true, "a"), live_rule("r2", true, "b")],
    );
    let before_count = ovr.singbox.rules.len();

    apply_template(&mut ovr, "custom:tpl-revert", 600).unwrap();
    assert_eq!(ovr.applied_templates.len(), 1);

    let reverted = revert_template(&mut ovr, "custom:tpl-revert");
    assert!(reverted);
    // Record removed; rules untouched; template definition untouched.
    assert!(ovr.applied_templates.is_empty());
    assert_eq!(ovr.singbox.rules.len(), before_count);
    assert_eq!(ovr.singbox.rules[0].id, "r1");
    assert_eq!(ovr.custom_templates.len(), 1);
}

#[test]
fn delete_custom_template_then_revert_still_works() {
    // J1 guarantee: revert consults applied_templates records only, so deleting
    // the template definition never breaks undoing an apply.
    let template = ref_template("tpl-del", "模板", &["r1"]);
    let mut ovr = ovr_with(&template, &[live_rule("r1", true, "a")]);

    apply_template(&mut ovr, "custom:tpl-del", 600).unwrap();
    // Delete the custom template definition.
    ovr.custom_templates.clear();

    let reverted = revert_template(&mut ovr, "custom:tpl-del");
    assert!(reverted);
    assert!(ovr.applied_templates.is_empty());
    assert_eq!(ovr.singbox.rules.len(), 1);
}

#[test]
fn revert_unknown_template_returns_false() {
    let mut ovr = empty_override();
    let reverted = revert_template(&mut ovr, "custom:missing");
    assert!(!reverted);
}

// ---------------------------------------------------------------------------
// Active rule ID set (union across applied templates)
// ---------------------------------------------------------------------------

#[test]
fn active_rule_ids_is_union_of_applied_template_refs() {
    let mut ovr = empty_override();
    ovr.singbox.rules = vec![live_rule("r1", true, "a"), live_rule("r2", true, "b")];
    ovr.custom_templates
        .push(ref_template("t1", "一", &["r1", "r2"]));
    ovr.custom_templates
        .push(ref_template("t2", "二", &["r2", "r3"]));

    // Nothing applied → empty active set.
    assert!(active_rule_ids(&ovr).is_empty());

    apply_template(&mut ovr, "custom:t1", 600).unwrap();
    let mut active = active_rule_ids(&ovr);
    assert_eq!(active.len(), 2);
    assert!(active.contains("r1"));
    assert!(active.contains("r2"));

    // Second template applied → union grows.
    apply_template(&mut ovr, "custom:t2", 700).unwrap();
    active = active_rule_ids(&ovr);
    assert_eq!(active.len(), 3);
    assert!(active.contains("r3"));

    // Reverting one shrinks the union.
    revert_template(&mut ovr, "custom:t1");
    active = active_rule_ids(&ovr);
    assert_eq!(active.len(), 2);
    assert!(active.contains("r2"));
    assert!(active.contains("r3"));
}

#[test]
fn active_rule_ids_ignores_builtin_records_and_missing_templates() {
    let mut ovr = empty_override();
    ovr.singbox.rules = vec![live_rule("r1", true, "a")];
    ovr.custom_templates.push(ref_template("t1", "一", &["r1"]));
    // Builtin-style record (no `custom:` prefix) contributes nothing.
    ovr.applied_templates.push(AppliedTemplate {
        template_id: "return-china".to_string(),
        applied_at: 1,
        generated_rule_ids: vec!["r1".to_string()],
    });
    // Applied record whose template was deleted contributes nothing.
    ovr.applied_templates.push(AppliedTemplate {
        template_id: "custom:ghost".to_string(),
        applied_at: 2,
        generated_rule_ids: Vec::new(),
    });

    let active = active_rule_ids(&ovr);
    assert!(active.is_empty());
}

// ---------------------------------------------------------------------------
// Invalid reference reporting (missing / disabled)
// ---------------------------------------------------------------------------

#[test]
fn template_invalid_refs_reports_missing_and_disabled() {
    let template = ref_template("tpl", "模板", &["r1", "r2", "r3", "r4"]);
    let mut ovr = empty_override();
    ovr.singbox.rules = vec![
        live_rule("r1", true, "ok"),   // valid
        live_rule("r2", false, "off"), // disabled → invalid
        live_rule("r3", true, "ok2"),  // exists + enabled → valid
                                       // r4 missing entirely → invalid.
    ];

    let invalid = template_invalid_refs(&ovr, &template);
    assert_eq!(invalid, vec!["r2", "r4"]);
}

#[test]
fn template_invalid_refs_empty_when_all_valid() {
    let template = ref_template("tpl", "模板", &["r1"]);
    let mut ovr = empty_override();
    ovr.singbox.rules = vec![live_rule("r1", true, "a")];
    assert!(template_invalid_refs(&ovr, &template).is_empty());
}
