//! Scenario template tests (split out of `template.rs` to stay within the
//! business-file size gate; see `.agents/rules/code-organization.md`).
//!
//! 自「废弃内置模板」起仅覆盖 custom-only 语义。

use super::*;
use crate::local_override::{RuleAction, RuleMatchType, RuleSetCategory, RuleSetSubscription};

fn empty_override() -> LocalOverride {
    LocalOverride::default()
}

/// Build a snapshot rule for a custom template.
fn snapshot_rule(
    name: &str,
    target: &str,
    match_type: RuleMatchType,
    action: RuleAction,
) -> LocalRule {
    LocalRule {
        id: uuid::Uuid::new_v4().to_string(),
        name: name.to_string(),
        enabled: true,
        match_type,
        target: target.to_string(),
        action,
        advanced: Default::default(),
        note: format!("snapshot {name}"),
        created_at: 100,
        sort_order: 7,
    }
}

fn sample_custom_template(id: &str, now: u64) -> CustomTemplate {
    CustomTemplate {
        id: id.to_string(),
        name: "我的模板".to_string(),
        desc: "规则组合快照".to_string(),
        rules: vec![
            snapshot_rule(
                "google proxy",
                "google.com",
                RuleMatchType::DomainSuffix,
                RuleAction::Proxy,
            ),
            snapshot_rule(
                "geoip-cn direct",
                "geoip-cn",
                RuleMatchType::RuleSet,
                RuleAction::Direct,
            ),
            snapshot_rule(
                "geosite-ads reject",
                "geosite-ads",
                RuleMatchType::RuleSet,
                RuleAction::Reject,
            ),
        ],
        created_at: now,
    }
}

fn ovr_with_template(template: &CustomTemplate) -> LocalOverride {
    let mut ovr = empty_override();
    ovr.custom_templates.push(template.clone());
    ovr
}

// ---------------------------------------------------------------------------
// Custom-only apply semantics
// ---------------------------------------------------------------------------

#[test]
fn apply_custom_template_copies_snapshot_rules_with_fresh_uuids() {
    let template = sample_custom_template("tpl-1", 500);
    let mut ovr = ovr_with_template(&template);

    let before = ovr.singbox.rules.len();
    let ids = apply_template(&mut ovr, "custom:tpl-1", 600).unwrap();

    // Rules copied with fresh UUIDs.
    assert_eq!(ovr.singbox.rules.len(), before + template.rules.len());
    assert_eq!(ids.len(), template.rules.len());
    assert!(
        ids.iter()
            .all(|id| !template.rules.iter().any(|r| &r.id == id))
    );

    // Field snapshot preserved (name / target / action / enabled / note).
    for (generated, snapshot) in ovr.singbox.rules.iter().skip(before).zip(&template.rules) {
        assert_eq!(generated.name, snapshot.name);
        assert_eq!(generated.target, snapshot.target);
        assert_eq!(generated.match_type, snapshot.match_type);
        assert_eq!(generated.action, snapshot.action);
        assert_eq!(generated.enabled, snapshot.enabled);
        assert_eq!(generated.note, snapshot.note);
        assert_eq!(generated.created_at, 600);
    }

    // Applied record uses "custom:<id>".
    assert_eq!(ovr.applied_templates.len(), 1);
    assert_eq!(ovr.applied_templates[0].template_id, "custom:tpl-1");
    assert_eq!(ovr.applied_templates[0].generated_rule_ids, ids);
}

#[test]
fn apply_custom_template_head_inserts_with_sort_order_delta() {
    // Pre-existing rules with sort_order up to 50 → generated rules get
    // sort_order starting at -100 (head semantics via sort weight).
    let template = sample_custom_template("tpl-head", 500);
    let mut ovr = ovr_with_template(&template);
    for i in 0..3 {
        let mut rule = snapshot_rule(
            "existing",
            "existing.com",
            RuleMatchType::Domain,
            RuleAction::Direct,
        );
        rule.sort_order = i * 25; // 0, 25, 50
        ovr.singbox.rules.push(rule);
    }
    let existing_count = ovr.singbox.rules.len();

    apply_template(&mut ovr, "custom:tpl-head", 600).unwrap();

    // Generated rules are appended with sort_order = base(min-100) + index.
    let generated: Vec<&LocalRule> = ovr.singbox.rules.iter().skip(existing_count).collect();
    let orders: Vec<i32> = generated.iter().map(|r| r.sort_order).collect();
    assert_eq!(orders, vec![-100, -99, -98]);
    assert!(
        generated
            .iter()
            .all(|r| template.rules.iter().any(|t| t.name == r.name))
    );

    // Original rules keep their place (and their original sort orders).
    let existing: Vec<&LocalRule> = ovr.singbox.rules.iter().take(existing_count).collect();
    assert_eq!(existing.len(), 3);
    assert!(existing.iter().all(|r| r.name == "existing"));
    let existing_orders: Vec<i32> = existing.iter().map(|r| r.sort_order).collect();
    assert_eq!(existing_orders, vec![0, 25, 50]);
}

#[test]
fn apply_custom_template_does_not_touch_rule_set_subscriptions() {
    // 不再自动订阅任何规则集：rule_set 快照规则原样复制（含 custom / 迁移后 tag）。
    let template = sample_custom_template("tpl-sub", 500);
    let mut ovr = ovr_with_template(&template);
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

    // 订阅条目未被标记 subscribed（机制已废弃）。
    assert!(ovr.rule_set_subscriptions.iter().all(|s| !s.subscribed));
    // rule_set 快照规则无条件生成。
    let rs_targets: Vec<&str> = ovr
        .singbox
        .rules
        .iter()
        .filter(|r| r.match_type == RuleMatchType::RuleSet)
        .map(|r| r.target.as_str())
        .collect();
    assert!(rs_targets.contains(&"geoip-cn"));
    assert!(rs_targets.contains(&"geosite-ads"));
}

#[test]
fn apply_custom_template_without_extra_segments_still_works() {
    // Bare override without subscription list: apply still succeeds.
    let template = sample_custom_template("tpl-bare", 500);
    let mut ovr = LocalOverride::default();
    ovr.custom_templates.push(template.clone());

    let ids = apply_template(&mut ovr, "custom:tpl-bare", 600).unwrap();
    assert_eq!(ids.len(), template.rules.len());
    assert_eq!(ovr.applied_templates.len(), 1);
}

#[test]
fn apply_template_rejects_builtin_or_unknown_ids() {
    let mut ovr = empty_override();

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
    assert!(ovr.singbox.rules.is_empty());
}

#[test]
fn apply_unknown_custom_template_errors() {
    let mut ovr = empty_override();
    let result = apply_template(&mut ovr, "custom:does-not-exist", 1000);
    assert!(result.is_err());
    assert!(ovr.applied_templates.is_empty());
}

#[test]
fn revert_template_removes_generated_rules() {
    let template = sample_custom_template("tpl-revert", 500);
    let mut ovr = ovr_with_template(&template);
    let before_count = ovr.singbox.rules.len();

    apply_template(&mut ovr, "custom:tpl-revert", 600).unwrap();
    assert_eq!(ovr.singbox.rules.len(), before_count + template.rules.len());

    let reverted = revert_template(&mut ovr, "custom:tpl-revert");
    assert!(reverted);
    assert_eq!(ovr.singbox.rules.len(), before_count);
    assert!(ovr.applied_templates.is_empty());
    // Revert does not touch the template definition.
    assert_eq!(ovr.custom_templates.len(), 1);
}

#[test]
fn delete_custom_template_then_revert_still_works() {
    // J1 guarantee: revert consults applied_templates records only, so deleting
    // the template definition never breaks undoing previously generated rules.
    let template = sample_custom_template("tpl-del", 500);
    let mut ovr = ovr_with_template(&template);
    let before_count = ovr.singbox.rules.len();

    apply_template(&mut ovr, "custom:tpl-del", 600).unwrap();
    // Delete the custom template definition.
    ovr.custom_templates.clear();

    let reverted = revert_template(&mut ovr, "custom:tpl-del");
    assert!(reverted);
    assert_eq!(ovr.singbox.rules.len(), before_count);
    assert!(ovr.applied_templates.is_empty());
}

#[test]
fn revert_unknown_template_returns_false() {
    let mut ovr = empty_override();
    let reverted = revert_template(&mut ovr, "custom:missing");
    assert!(!reverted);
}

#[test]
fn custom_template_reapply_generates_distinct_uuids() {
    let template = sample_custom_template("tpl-twice", 500);
    let mut ovr = ovr_with_template(&template);

    let first = apply_template(&mut ovr, "custom:tpl-twice", 600).unwrap();
    let second = apply_template(&mut ovr, "custom:tpl-twice", 700).unwrap();
    assert_eq!(ovr.applied_templates.len(), 2);

    let overlap: Vec<_> = first.iter().filter(|id| second.contains(id)).collect();
    assert!(overlap.is_empty());
}
