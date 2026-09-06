//! Scenario template tests (split out of `template.rs` to stay within the
//! business-file size gate; see `.agents/rules/code-organization.md`).

use super::*;
use crate::local_override::store::built_in_rule_set_subscriptions;

fn empty_override() -> LocalOverride {
    LocalOverride {
        rule_set_subscriptions: built_in_rule_set_subscriptions(),
        ..Default::default()
    }
}

fn is_subscribed_to(ovr: &LocalOverride, community_id: &str) -> bool {
    ovr.rule_set_subscriptions
        .iter()
        .any(|s| s.community_id == community_id && s.subscribed)
}

fn rule_set_rules(ovr: &LocalOverride) -> impl Iterator<Item = &LocalRule> + '_ {
    ovr.singbox
        .rules
        .iter()
        .filter(|r| r.match_type == RuleMatchType::RuleSet)
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
        desc: "内置依赖自动订阅".to_string(),
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
            snapshot_rule(
                "custom tag proxy",
                "my-custom-set",
                RuleMatchType::RuleSet,
                RuleAction::Proxy,
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
// Built-in behavior (unchanged from inline tests)
// ---------------------------------------------------------------------------

#[test]
fn template_dependency_mapping() {
    assert_eq!(
        template_rule_set_dependencies(TEMPLATE_RETURN_CHINA),
        &["geoip-cn", "geosite-cn"]
    );
    assert_eq!(
        template_rule_set_dependencies(TEMPLATE_OVERSEAS),
        &["geosite-geolocation-!cn"]
    );
    assert_eq!(
        template_rule_set_dependencies(TEMPLATE_AD_FILTER),
        &["geosite-ads"]
    );
    assert!(template_rule_set_dependencies("unknown").is_empty());
}

#[test]
fn apply_return_china_template_generates_rules() {
    // Not pre-subscribed: apply must still generate the rule_set rules and
    // auto-subscribe the dependency rule sets.
    let mut ovr = empty_override();
    assert!(!is_subscribed_to(&ovr, "geoip-cn"));
    assert!(!is_subscribed_to(&ovr, "geosite-cn"));

    let ids = apply_template(&mut ovr, TEMPLATE_RETURN_CHINA, 1000).unwrap();
    assert!(!ids.is_empty());
    assert_eq!(ovr.singbox.rules.len(), ids.len());
    assert_eq!(ovr.applied_templates.len(), 1);
    assert_eq!(ovr.applied_templates[0].template_id, TEMPLATE_RETURN_CHINA);

    let rs_targets: Vec<&str> = rule_set_rules(&ovr).map(|r| r.target.as_str()).collect();
    assert!(rs_targets.contains(&"geoip-cn"));
    assert!(rs_targets.contains(&"geosite-cn"));
}

#[test]
fn apply_overseas_template_generates_rules() {
    let mut ovr = empty_override();

    let ids = apply_template(&mut ovr, TEMPLATE_OVERSEAS, 2000).unwrap();
    assert!(!ids.is_empty());
    assert!(ovr.singbox.rules.iter().any(|r| r.target == "google.com"));
    assert!(ovr.singbox.rules.iter().any(|r| r.target == "youtube.com"));
    assert!(ovr.singbox.rules.iter().any(|r| r.target == "github.com"));
    assert!(rule_set_rules(&ovr).any(|r| r.target == "geosite-geolocation-!cn"));
}

#[test]
fn apply_ad_filter_template_generates_rules() {
    // Regression: previously unsubscribed ad-filter generated 0 rules.
    let mut ovr = empty_override();

    let ids = apply_template(&mut ovr, TEMPLATE_AD_FILTER, 3000).unwrap();
    assert!(!ids.is_empty());
    assert_eq!(ids.len(), 1);
    let reject =
        rule_set_rules(&ovr).find(|r| r.target == "geosite-ads" && r.action == RuleAction::Reject);
    assert!(reject.is_some());
}

#[test]
fn apply_template_auto_subscribes_dependency_rule_sets() {
    // return-china → geoip-cn + geosite-cn.
    let mut ovr = empty_override();
    apply_template(&mut ovr, TEMPLATE_RETURN_CHINA, 1000).unwrap();
    assert!(is_subscribed_to(&ovr, "geoip-cn"));
    assert!(is_subscribed_to(&ovr, "geosite-cn"));

    // overseas → geosite-geolocation-!cn.
    let mut ovr = empty_override();
    apply_template(&mut ovr, TEMPLATE_OVERSEAS, 2000).unwrap();
    assert!(is_subscribed_to(&ovr, "geosite-geolocation-!cn"));

    // ad-filter → geosite-ads.
    let mut ovr = empty_override();
    apply_template(&mut ovr, TEMPLATE_AD_FILTER, 3000).unwrap();
    assert!(is_subscribed_to(&ovr, "geosite-ads"));

    // Unknown templates subscribe nothing.
    let mut ovr = empty_override();
    assert!(apply_template(&mut ovr, "unknown", 1000).is_err());
    assert!(ovr.rule_set_subscriptions.iter().all(|s| !s.subscribed));
}

#[test]
fn revert_template_removes_generated_rules() {
    let mut ovr = empty_override();

    let before_count = ovr.singbox.rules.len();
    apply_template(&mut ovr, TEMPLATE_RETURN_CHINA, 1000).unwrap();
    assert!(ovr.singbox.rules.len() > before_count);
    assert!(is_subscribed_to(&ovr, "geoip-cn"));

    let reverted = revert_template(&mut ovr, TEMPLATE_RETURN_CHINA);
    assert!(reverted);
    assert_eq!(ovr.singbox.rules.len(), before_count);
    assert!(ovr.applied_templates.is_empty());

    // Revert only removes generated rules; re-apply works normally.
    let ids = apply_template(&mut ovr, TEMPLATE_RETURN_CHINA, 2000).unwrap();
    assert!(!ids.is_empty());
    assert!(
        ovr.applied_templates
            .iter()
            .any(|t| t.template_id == TEMPLATE_RETURN_CHINA)
    );
}

#[test]
fn revert_unknown_template_returns_false() {
    let mut ovr = empty_override();
    let reverted = revert_template(&mut ovr, "nonexistent");
    assert!(!reverted);
}

#[test]
fn apply_unknown_template_errors() {
    let mut ovr = empty_override();
    let result = apply_template(&mut ovr, "unknown", 1000);
    assert!(result.is_err());
}

#[test]
fn template_rules_have_unique_ids() {
    let mut ovr = empty_override();

    let ids = apply_template(&mut ovr, TEMPLATE_RETURN_CHINA, 1000).unwrap();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique.len(), ids.len());
}

// ---------------------------------------------------------------------------
// Custom templates
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
    assert_eq!(orders, vec![-100, -99, -98, -97]);
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
fn apply_custom_template_auto_subscribes_builtin_rule_set_dependencies() {
    let template = sample_custom_template("tpl-sub", 500);
    let mut ovr = ovr_with_template(&template);
    assert!(!is_subscribed_to(&ovr, "geoip-cn"));
    assert!(!is_subscribed_to(&ovr, "geosite-ads"));

    apply_template(&mut ovr, "custom:tpl-sub", 600).unwrap();

    // Snapshot rule_set targets matching a built-in community id are subscribed.
    assert!(is_subscribed_to(&ovr, "geoip-cn"));
    assert!(is_subscribed_to(&ovr, "geosite-ads"));

    // A custom rule set tag must NOT be treated as a subscription.
    assert!(
        ovr.rule_set_subscriptions
            .iter()
            .all(|s| s.community_id != "my-custom-set")
    );

    // Rule_set rules generated unconditionally.
    let rs_targets: Vec<&str> = rule_set_rules(&ovr).map(|r| r.target.as_str()).collect();
    assert!(rs_targets.contains(&"geoip-cn"));
    assert!(rs_targets.contains(&"geosite-ads"));
    assert!(rs_targets.contains(&"my-custom-set"));
}

#[test]
fn apply_custom_template_without_subscription_list_skips_silently() {
    // Legacy / bare override without a subscription list: apply still succeeds
    // and generates all rules (nothing to subscribe is a no-op).
    let template = sample_custom_template("tpl-bare", 500);
    let mut ovr = LocalOverride::default();
    ovr.custom_templates.push(template.clone());

    let ids = apply_template(&mut ovr, "custom:tpl-bare", 600).unwrap();
    assert_eq!(ids.len(), template.rules.len());
    assert_eq!(ovr.applied_templates.len(), 1);
}

#[test]
fn apply_unknown_custom_template_errors() {
    let mut ovr = empty_override();
    let result = apply_template(&mut ovr, "custom:does-not-exist", 1000);
    assert!(result.is_err());
    assert!(ovr.applied_templates.is_empty());
}

#[test]
fn apply_custom_template_keeps_builtin_flow_intact() {
    // A plain id that is not built-in still errors (no accidental custom path).
    let mut ovr = empty_override();
    assert!(apply_template(&mut ovr, "custom", 1000).is_err());
    assert!(apply_template(&mut ovr, "custom:", 1000).is_err());
}

#[test]
fn revert_custom_template_removes_generated_rules() {
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
fn template_auto_subscribed_community_ids_reports_custom_deps() {
    let template = sample_custom_template("tpl-deps", 500);
    let ovr = ovr_with_template(&template);

    // Custom path reports only snapshot rule_set targets that match a built-in
    // subscription community id (dedup, stable order).
    let ids = template_auto_subscribed_community_ids(&ovr, "custom:tpl-deps");
    assert_eq!(ids, vec!["geoip-cn".to_string(), "geosite-ads".to_string()]);

    // Built-in path reports the static dependency map.
    let builtin = template_auto_subscribed_community_ids(&ovr, TEMPLATE_RETURN_CHINA);
    assert_eq!(
        builtin,
        vec!["geoip-cn".to_string(), "geosite-cn".to_string()]
    );

    // Unknown custom id → empty (apply itself errors first).
    assert!(template_auto_subscribed_community_ids(&ovr, "custom:missing").is_empty());
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
