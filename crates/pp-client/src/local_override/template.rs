//! Scenario templates: built-in (return-china, overseas, ad-filter) plus
//! user-defined custom templates ([`CustomTemplate`]).
//!
//! Applying a template auto-subscribes the community rule sets it depends on
//! (built-ins via [`template_rule_set_dependencies`]; custom templates by
//! scanning their snapshot rules for `rule_set` targets that match a built-in
//! subscription `community_id`), then generates multiple [`LocalRule`] entries
//! and records an [`AppliedTemplate`] for revert.
//!
//! Custom templates are addressed with the `"custom:"` prefix
//! ([`CUSTOM_TEMPLATE_PREFIX`]) so they share the apply / revert command path
//! with built-in templates; [`revert_template`] only consults the recorded
//! `generated_rule_ids`, so removing a custom template never breaks the revert
//! of rules it previously generated.
//!
//! ADR-0002, section 3.3.

use std::collections::HashSet;

use pp_common::{PanelError, PanelResult};

use super::{AppliedTemplate, CustomTemplate, LocalOverride, LocalRule, RuleAction, RuleMatchType};

/// Prefix distinguishing a user-defined template ID from built-in ones.
///
/// Apply / revert commands receive `"custom:<id>"` as the `template_id`.
pub const CUSTOM_TEMPLATE_PREFIX: &str = "custom:";

/// Built-in template identifiers.
pub const TEMPLATE_RETURN_CHINA: &str = "return-china";
pub const TEMPLATE_OVERSEAS: &str = "overseas";
pub const TEMPLATE_AD_FILTER: &str = "ad-filter";

/// All built-in template IDs.
pub const BUILT_IN_TEMPLATE_IDS: &[&str] =
    &[TEMPLATE_RETURN_CHINA, TEMPLATE_OVERSEAS, TEMPLATE_AD_FILTER];

/// Community rule sets a built-in template depends on.
///
/// Applying a template auto-subscribes these rule sets (the generated
/// `rule_set` rules resolve only after the rule sets are downloaded), per
/// ADR-0002 section 3.3.3.
///
/// Returns an empty slice for unknown template IDs.
pub fn template_rule_set_dependencies(template_id: &str) -> &'static [&'static str] {
    match template_id {
        TEMPLATE_RETURN_CHINA => &["geoip-cn", "geosite-cn"],
        TEMPLATE_OVERSEAS => &["geosite-geolocation-!cn"],
        TEMPLATE_AD_FILTER => &["geosite-ads"],
        _ => &[],
    }
}

/// Community rule set ids that applying `template_id` auto-subscribes.
///
/// - Built-ins: the static dependency map ([`template_rule_set_dependencies`]).
/// - Custom (`"custom:<id>"`): the `rule_set` targets of the template's
///   snapshot rules that match an entry in `rule_set_subscriptions` (i.e. a
///   built-in community id). Custom rule set tags are **not** subscribed —
///   they only need their own rule set to be enabled.
///
/// Used by the command layer to trigger best-effort downloads after an apply.
/// Returns ids in stable (definition) order with duplicates removed.
pub fn template_auto_subscribed_community_ids(
    ovr: &LocalOverride,
    template_id: &str,
) -> Vec<String> {
    match template_id.strip_prefix(CUSTOM_TEMPLATE_PREFIX) {
        Some(custom_id) => {
            let Some(tpl) = ovr.custom_templates.iter().find(|t| t.id == custom_id) else {
                return Vec::new();
            };
            let subscribed: HashSet<&str> = ovr
                .rule_set_subscriptions
                .iter()
                .map(|s| s.community_id.as_str())
                .collect();
            let mut seen = HashSet::new();
            tpl.rules
                .iter()
                .filter(|r| {
                    r.match_type == RuleMatchType::RuleSet
                        && subscribed.contains(r.target.as_str())
                        && seen.insert(r.target.as_str())
                })
                .map(|r| r.target.clone())
                .collect()
        }
        None => template_rule_set_dependencies(template_id)
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

/// Mark the built-in community rule sets referenced by `rule_set` rules as
/// subscribed. Targets not present in `rule_set_subscriptions` (e.g. custom
/// rule set tags) are skipped defensively.
fn auto_subscribe_rule_set_targets(ovr: &mut LocalOverride, rules: &[LocalRule]) {
    for rule in rules {
        if rule.match_type != RuleMatchType::RuleSet {
            continue;
        }
        if let Some(sub) = ovr
            .rule_set_subscriptions
            .iter_mut()
            .find(|s| s.community_id == rule.target)
        {
            sub.subscribed = true;
        } else {
            tracing::debug!(
                target = %rule.target,
                "template rule_set target not in subscription list, skipping"
            );
        }
    }
}

/// Apply a scenario template to the given [`LocalOverride`].
///
/// Built-in templates:
/// - Auto-subscribes the community rule sets the template depends on
///   ([`template_rule_set_dependencies`]); IDs not present in
///   `rule_set_subscriptions` are skipped defensively.
/// - Generates rules with fresh UUIDs; the `rule_set` rules are always
///   generated, regardless of prior subscription state.
/// - Inserts rules at the head of the list (`sort_order = min - 100`).
/// - Records the applied template for later revert.
/// - Returns the generated rule IDs.
///
/// Custom templates (`template_id = "custom:<id>"`, [`CUSTOM_TEMPLATE_PREFIX`]):
/// copies the snapshot rules with fresh UUIDs, auto-subscribes the built-in
/// community rule sets their `rule_set` rules reference, head-inserts them and
/// records `applied_templates` with `template_id = "custom:<id>"`.
///
/// # Errors
///
/// Returns error if `template_id` is neither a known built-in nor an existing
/// custom template.
pub fn apply_template(
    ovr: &mut LocalOverride,
    template_id: &str,
    now_sec: u64,
) -> PanelResult<Vec<String>> {
    if template_id.starts_with(CUSTOM_TEMPLATE_PREFIX) {
        return apply_custom_template(ovr, template_id, now_sec);
    }

    // Auto-subscribe the dependency rule sets so the generated rule_set rules
    // take effect once the rule sets are downloaded. Unknown IDs are skipped
    // (the built-in list covers every dependency, so this is only defensive).
    for &community_id in template_rule_set_dependencies(template_id) {
        if let Some(sub) = ovr
            .rule_set_subscriptions
            .iter_mut()
            .find(|s| s.community_id.as_str() == community_id)
        {
            sub.subscribed = true;
        } else {
            tracing::debug!(
                community_id,
                "template dependency rule set not in subscription list, skipping"
            );
        }
    }

    let generated = match template_id {
        TEMPLATE_RETURN_CHINA => generate_return_china_rules(now_sec),
        TEMPLATE_OVERSEAS => generate_overseas_rules(now_sec),
        TEMPLATE_AD_FILTER => generate_ad_filter_rules(now_sec),
        _ => {
            return Err(PanelError::Client(format!(
                "unknown template id: {template_id}"
            )));
        }
    };

    let rule_ids: Vec<String> = generated.iter().map(|r| r.id.clone()).collect();

    // Insert generated rules.
    let min_order = min_sort_order(&ovr.singbox.rules);
    let base_order = min_order.saturating_sub(100);

    for (i, mut rule) in generated.into_iter().enumerate() {
        rule.sort_order = base_order + i as i32;
        ovr.singbox.rules.push(rule);
    }

    // Record applied template.
    ovr.applied_templates.push(AppliedTemplate {
        template_id: template_id.to_string(),
        applied_at: now_sec,
        generated_rule_ids: rule_ids.clone(),
    });

    Ok(rule_ids)
}

/// Apply a user-defined custom template (addressed as `"custom:<id>"`).
///
/// - Looks the template up in `ovr.custom_templates` (errors if missing).
/// - Auto-subscribes built-in community rule sets referenced by the snapshot's
///   `rule_set` rules ([`auto_subscribe_rule_set_targets`]).
/// - Copies the snapshot rules with **fresh UUIDs**, head-inserts them
///   (`sort_order = min - 100`, preserving snapshot order), and records an
///   [`AppliedTemplate`] with `template_id = "custom:<id>"`.
fn apply_custom_template(
    ovr: &mut LocalOverride,
    template_id: &str,
    now_sec: u64,
) -> PanelResult<Vec<String>> {
    let Some(custom_id) = template_id.strip_prefix(CUSTOM_TEMPLATE_PREFIX) else {
        return Err(PanelError::Client(format!(
            "malformed custom template id: {template_id}"
        )));
    };
    let template: CustomTemplate = ovr
        .custom_templates
        .iter()
        .find(|t| t.id == custom_id)
        .cloned()
        .ok_or_else(|| PanelError::Client(format!("custom template not found: {custom_id}")))?;

    auto_subscribe_rule_set_targets(ovr, &template.rules);

    let min_order = min_sort_order(&ovr.singbox.rules);
    let base_order = min_order.saturating_sub(100);

    let generated: Vec<LocalRule> = template
        .rules
        .into_iter()
        .enumerate()
        .map(|(i, mut rule)| {
            rule.id = uuid::Uuid::new_v4().to_string();
            rule.created_at = now_sec;
            rule.sort_order = base_order + i as i32;
            rule
        })
        .collect();
    let rule_ids: Vec<String> = generated.iter().map(|r| r.id.clone()).collect();

    ovr.singbox.rules.extend(generated);

    ovr.applied_templates.push(AppliedTemplate {
        template_id: template_id.to_string(),
        applied_at: now_sec,
        generated_rule_ids: rule_ids.clone(),
    });

    Ok(rule_ids)
}

/// Revert (undo) a previously applied template by its ID.
///
/// Removes all rules whose IDs were generated by the template, and removes
/// the [`AppliedTemplate`] record.
///
/// Returns `true` if a template was found and reverted.
pub fn revert_template(ovr: &mut LocalOverride, template_id: &str) -> bool {
    let Some(idx) = ovr
        .applied_templates
        .iter()
        .position(|t| t.template_id == template_id)
    else {
        return false;
    };

    let record = ovr.applied_templates.remove(idx);
    let ids_to_remove: HashSet<String> = record.generated_rule_ids.into_iter().collect();

    ovr.singbox.rules.retain(|r| !ids_to_remove.contains(&r.id));

    true
}

// ---------------------------------------------------------------------------
// Template generators
// ---------------------------------------------------------------------------

/// Return-China template:
/// 1. domain_suffix: [cn, com.cn, net.cn] → direct
/// 2. rule_set: geoip-cn → direct
/// 3. rule_set: geosite-cn → direct
fn generate_return_china_rules(now_sec: u64) -> Vec<LocalRule> {
    // rule_set references are always generated; the dependency rule sets are
    // auto-subscribed when the template is applied.
    vec![
        LocalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: "China TLD direct".to_string(),
            enabled: true,
            match_type: RuleMatchType::DomainSuffix,
            target: "cn".to_string(),
            action: RuleAction::Direct,
            advanced: Default::default(),
            note: "Auto-generated by return-china template".to_string(),
            created_at: now_sec,
            sort_order: 0,
        },
        LocalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: "com.cn direct".to_string(),
            enabled: true,
            match_type: RuleMatchType::DomainSuffix,
            target: "com.cn".to_string(),
            action: RuleAction::Direct,
            advanced: Default::default(),
            note: "Auto-generated by return-china template".to_string(),
            created_at: now_sec,
            sort_order: 0,
        },
        LocalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: "net.cn direct".to_string(),
            enabled: true,
            match_type: RuleMatchType::DomainSuffix,
            target: "net.cn".to_string(),
            action: RuleAction::Direct,
            advanced: Default::default(),
            note: "Auto-generated by return-china template".to_string(),
            created_at: now_sec,
            sort_order: 0,
        },
        LocalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: "GeoIP CN direct".to_string(),
            enabled: true,
            match_type: RuleMatchType::RuleSet,
            target: "geoip-cn".to_string(),
            action: RuleAction::Direct,
            advanced: Default::default(),
            note: "Auto-generated by return-china template".to_string(),
            created_at: now_sec,
            sort_order: 0,
        },
        LocalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: "GeoSite CN direct".to_string(),
            enabled: true,
            match_type: RuleMatchType::RuleSet,
            target: "geosite-cn".to_string(),
            action: RuleAction::Direct,
            advanced: Default::default(),
            note: "Auto-generated by return-china template".to_string(),
            created_at: now_sec,
            sort_order: 0,
        },
    ]
}

/// Overseas template:
/// 1. domain_suffix: [google.com, youtube.com, github.com] → proxy
/// 2. rule_set: geosite-geolocation-!cn → proxy
fn generate_overseas_rules(now_sec: u64) -> Vec<LocalRule> {
    let mut rules = Vec::new();

    for domain in ["google.com", "youtube.com", "github.com"] {
        rules.push(LocalRule {
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("{domain} proxy"),
            enabled: true,
            match_type: RuleMatchType::DomainSuffix,
            target: domain.to_string(),
            action: RuleAction::Proxy,
            advanced: Default::default(),
            note: "Auto-generated by overseas template".to_string(),
            created_at: now_sec,
            sort_order: 0,
        });
    }

    // rule_set reference is always generated; the dependency rule set is
    // auto-subscribed when the template is applied.
    rules.push(LocalRule {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Non-China geolocation proxy".to_string(),
        enabled: true,
        match_type: RuleMatchType::RuleSet,
        target: "geosite-geolocation-!cn".to_string(),
        action: RuleAction::Proxy,
        advanced: Default::default(),
        note: "Auto-generated by overseas template".to_string(),
        created_at: now_sec,
        sort_order: 0,
    });

    rules
}

/// Ad-Filter template:
/// 1. rule_set: geosite-ads → reject
/// 2. rule_set: geosite-category-ads-all → reject
fn generate_ad_filter_rules(now_sec: u64) -> Vec<LocalRule> {
    // rule_set reference is always generated; the dependency rule set is
    // auto-subscribed when the template is applied.
    vec![LocalRule {
        id: uuid::Uuid::new_v4().to_string(),
        name: "Ad domains reject".to_string(),
        enabled: true,
        match_type: RuleMatchType::RuleSet,
        target: "geosite-ads".to_string(),
        action: RuleAction::Reject,
        advanced: Default::default(),
        note: "Auto-generated by ad-filter template".to_string(),
        created_at: now_sec,
        sort_order: 0,
    }]
    // geosite-category-ads-all is the same as geosite-ads in our built-in list;
    // we use the community_id "geosite-ads" which maps to category-ads-all.
    // Skip duplicate if same.
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn min_sort_order(rules: &[LocalRule]) -> i32 {
    rules.iter().map(|r| r.sort_order).min().unwrap_or(0)
}

#[cfg(test)]
#[path = "tests/template_tests.rs"]
mod tests;
