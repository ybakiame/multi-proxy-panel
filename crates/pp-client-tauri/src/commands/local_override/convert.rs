//! Conversion and validation helpers for local override commands.

use pp_client::local_override::{
    AppliedTemplate, CoreLocalOverride, CustomTemplate, LocalOverride, LocalRule,
};

use super::views::*;

/// Validate local override before saving.
///
/// Checks:
/// - Rule IDs are unique.
/// - Targets are non-empty for non-Final rules.
/// - RuleSet references resolve to an **enabled custom rule set** tag
///   （内置社区订阅已废弃，规则集引用统一指向用户自控的 custom set）。
/// - Custom rule set IDs/tags are unique; Remote URLs / Manual contents are
///   non-empty.
pub(super) fn validate_local_override(ovr: &LocalOverride) -> Result<(), String> {
    let core_ovr = &ovr.singbox;
    // Check rule ID uniqueness.
    let mut seen = std::collections::HashSet::new();
    for rule in &core_ovr.rules {
        if !seen.insert(rule.id.clone()) {
            return Err(format!("duplicate rule id '{}'", rule.id));
        }
    }

    // Check non-empty targets.
    for rule in &core_ovr.rules {
        if !matches!(
            rule.match_type,
            pp_client::local_override::RuleMatchType::Final
        ) && rule.target.trim().is_empty()
        {
            return Err(format!("rule '{}' has empty target", rule.id));
        }
    }

    // Check RuleSet references resolve to an enabled custom rule set tag.
    for rule in &core_ovr.rules {
        if matches!(
            rule.match_type,
            pp_client::local_override::RuleMatchType::RuleSet
        ) {
            let custom_enabled = ovr
                .custom_rule_sets
                .iter()
                .any(|rs| rs.tag == rule.target && rs.enabled);
            if !custom_enabled {
                return Err(format!(
                    "rule '{}' references unavailable rule set '{}' \
                     (need an enabled custom rule set with this tag)",
                    rule.id, rule.target
                ));
            }
        }
    }

    validate_custom_rule_sets(ovr)?;

    validate_custom_templates(ovr)?;

    Ok(())
}

/// Custom rule set segment validation:
/// - unique IDs;
/// - tags non-empty and unique among custom rule sets;
/// - Remote URL / Manual content non-empty.
pub(super) fn validate_custom_rule_sets(ovr: &LocalOverride) -> Result<(), String> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_tags = std::collections::HashSet::new();
    for rs in &ovr.custom_rule_sets {
        if !seen_ids.insert(rs.id.clone()) {
            return Err(format!("duplicate custom rule set id '{}'", rs.id));
        }
        let tag = rs.tag.trim();
        if tag.is_empty() {
            return Err(format!("custom rule set '{}' has an empty tag", rs.id));
        }
        if !seen_tags.insert(rs.tag.clone()) {
            return Err(format!("duplicate custom rule set tag '{}'", rs.tag));
        }
        match &rs.source {
            pp_client::local_override::CustomRuleSetSource::Remote { url, .. } => {
                if url.trim().is_empty() {
                    return Err(format!("custom rule set '{}' has an empty URL", rs.id));
                }
            }
            pp_client::local_override::CustomRuleSetSource::Manual { content } => {
                if content.trim().is_empty() {
                    return Err(format!("custom rule set '{}' has empty content", rs.id));
                }
            }
        }
    }

    Ok(())
}

/// Custom template segment validation:
/// - IDs unique, non-empty and must not carry the reserved `custom:` prefix
///   (apply addresses custom templates as `"custom:<id>"`, see template.rs);
/// - snapshot rule IDs within one template are unique (apply assigns fresh
///   UUIDs, but a consistent snapshot avoids confusion).
pub(super) fn validate_custom_templates(ovr: &LocalOverride) -> Result<(), String> {
    let prefix = pp_client::local_override::CUSTOM_TEMPLATE_PREFIX;
    let mut seen_ids = std::collections::HashSet::new();
    for tpl in &ovr.custom_templates {
        if tpl.id.trim().is_empty() {
            return Err("custom template has an empty id".to_string());
        }
        if tpl.id.starts_with(prefix) {
            return Err(format!(
                "custom template id '{}' must not start with reserved prefix '{prefix}'",
                tpl.id
            ));
        }
        if !seen_ids.insert(tpl.id.clone()) {
            return Err(format!("duplicate custom template id '{}'", tpl.id));
        }

        let mut seen_rule_ids = std::collections::HashSet::new();
        for rule in &tpl.rules {
            if !seen_rule_ids.insert(rule.id.clone()) {
                return Err(format!(
                    "custom template '{}' contains duplicate snapshot rule id '{}'",
                    tpl.id, rule.id
                ));
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn convert_input_to_model(
    input: SaveLocalOverrideInput,
) -> Result<LocalOverride, String> {
    Ok(LocalOverride {
        singbox: convert_core_input(input.singbox)?,
        // 内置订阅段已废弃：save 恒写空数组（serde 兼容旧字段，空数组无害）。
        rule_set_subscriptions: Vec::new(),
        applied_templates: input
            .applied_templates
            .into_iter()
            .map(convert_applied_template_input)
            .collect(),
        custom_rule_sets: input.custom_rule_sets,
        custom_templates: input
            .custom_templates
            .into_iter()
            .map(convert_custom_template_input)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn convert_custom_template_input(input: CustomTemplateInput) -> Result<CustomTemplate, String> {
    Ok(CustomTemplate {
        id: input.id,
        name: input.name,
        desc: input.desc,
        rules: input
            .rules
            .into_iter()
            .map(convert_rule_input)
            .collect::<Result<Vec<_>, _>>()?,
        created_at: input.created_at,
    })
}

pub(super) fn convert_core_input(
    input: CoreLocalOverrideInput,
) -> Result<CoreLocalOverride, String> {
    Ok(CoreLocalOverride {
        rules: input
            .rules
            .into_iter()
            .map(convert_rule_input)
            .collect::<Result<Vec<_>, _>>()?,
        rule_sets: input
            .rule_sets
            .into_iter()
            .map(convert_rule_set_ref_input)
            .collect::<Result<Vec<_>, _>>()?,
        enabled: input.enabled,
    })
}

fn convert_rule_input(input: LocalRuleInput) -> Result<LocalRule, String> {
    let match_type = parse_match_type(&input.match_type)?;
    let action = parse_action(&input.action)?;
    Ok(LocalRule {
        id: input.id,
        name: input.name,
        enabled: input.enabled,
        match_type,
        target: input.target,
        action,
        advanced: pp_client::local_override::RuleAdvancedOptions {
            no_resolve: input.no_resolve,
            invert: input.invert,
            _sniff: false,
        },
        note: input.note,
        created_at: input.created_at,
        sort_order: input.sort_order,
    })
}

fn parse_match_type(s: &str) -> Result<pp_client::local_override::RuleMatchType, String> {
    match s {
        "domain" => Ok(pp_client::local_override::RuleMatchType::Domain),
        "domain_suffix" => Ok(pp_client::local_override::RuleMatchType::DomainSuffix),
        "domain_keyword" => Ok(pp_client::local_override::RuleMatchType::DomainKeyword),
        "ip_cidr" => Ok(pp_client::local_override::RuleMatchType::IpCidr),
        "source_ip_cidr" => Ok(pp_client::local_override::RuleMatchType::SourceIpCidr),
        "rule_set" => Ok(pp_client::local_override::RuleMatchType::RuleSet),
        #[cfg(target_os = "android")]
        "app_package" => Ok(pp_client::local_override::RuleMatchType::AppPackage),
        #[cfg(not(target_os = "android"))]
        "process_name" => Ok(pp_client::local_override::RuleMatchType::ProcessName),
        "port" => Ok(pp_client::local_override::RuleMatchType::Port),
        "final" => Ok(pp_client::local_override::RuleMatchType::Final),
        _ => Err(format!("unknown match_type: {s}")),
    }
}

fn parse_action(s: &str) -> Result<pp_client::local_override::RuleAction, String> {
    match s {
        "proxy" => Ok(pp_client::local_override::RuleAction::Proxy),
        "direct" => Ok(pp_client::local_override::RuleAction::Direct),
        "reject" => Ok(pp_client::local_override::RuleAction::Reject),
        _ => {
            if let Some(tag) = s.strip_prefix("outbound:") {
                Ok(pp_client::local_override::RuleAction::Outbound {
                    tag: tag.to_string(),
                })
            } else {
                Err(format!("unknown action: {s}"))
            }
        }
    }
}

fn convert_rule_set_ref_input(
    input: LocalRuleSetRefInput,
) -> Result<pp_client::local_override::LocalRuleSetRef, String> {
    let kind = parse_rule_set_kind(&input.kind)?;
    let source = pp_client::local_override::RuleSetSource::Remote { url: input.source };
    Ok(pp_client::local_override::LocalRuleSetRef {
        id: input.id,
        name: input.name,
        tag: input.tag,
        kind,
        source,
        enabled: input.enabled,
        auto_update_interval_minutes: input.auto_update_interval_minutes,
        last_updated: input.last_updated,
    })
}

fn parse_rule_set_kind(s: &str) -> Result<pp_client::local_override::RuleSetKind, String> {
    match s {
        "singbox_remote" => Ok(pp_client::local_override::RuleSetKind::SingBoxRemote),
        "singbox_local" => Ok(pp_client::local_override::RuleSetKind::SingBoxLocal),
        _ => Err(format!("unknown rule_set kind: {s}")),
    }
}

fn convert_applied_template_input(input: AppliedTemplateInput) -> AppliedTemplate {
    AppliedTemplate {
        template_id: input.template_id,
        applied_at: input.applied_at,
        generated_rule_ids: input.generated_rule_ids,
    }
}

// ---------------------------------------------------------------------------
// Round-trip tests: model → view → input → model
// ---------------------------------------------------------------------------
//
// The frontend receives `LocalRuleView` (string-typed enums), then echoes the
// exact same strings back as `LocalRuleInput` / `LocalRuleSetRefInput`. The
// view strings must therefore be accepted by the `parse_*` functions below, or
// saves fail with `unknown match_type` / `unknown action` / `unknown rule_set
// kind` (regression: `format!("{:?}").to_lowercase()` emitted `"ruleset"`,
// `"domainsuffix"`, … which `parse_match_type` rejected).

#[cfg(test)]
mod roundtrip_tests {
    use super::*;

    fn sample_rule(match_type: pp_client::local_override::RuleMatchType) -> LocalRule {
        LocalRule {
            id: "r1".to_string(),
            name: "rule".to_string(),
            enabled: true,
            match_type,
            target: "example.com".to_string(),
            action: pp_client::local_override::RuleAction::Proxy,
            advanced: Default::default(),
            note: String::new(),
            created_at: 1,
            sort_order: 0,
        }
    }

    /// `LocalRule` → `LocalRuleView` → `LocalRuleInput` → `LocalRule`.
    fn rule_round_trip(model: LocalRule) -> LocalRule {
        let view = LocalRuleView::from_model(&model);
        let input = LocalRuleInput {
            id: view.id,
            name: view.name,
            enabled: view.enabled,
            match_type: view.match_type,
            target: view.target,
            action: view.action,
            no_resolve: view.no_resolve,
            invert: view.invert,
            note: view.note,
            created_at: view.created_at,
            sort_order: view.sort_order,
        };
        convert_rule_input(input).unwrap()
    }

    #[test]
    fn every_match_type_round_trips_through_view_string() {
        use pp_client::local_override::RuleMatchType as M;
        let cases: Vec<(M, &str)> = vec![
            (M::Domain, "domain"),
            (M::DomainSuffix, "domain_suffix"),
            (M::DomainKeyword, "domain_keyword"),
            (M::IpCidr, "ip_cidr"),
            (M::SourceIpCidr, "source_ip_cidr"),
            (M::RuleSet, "rule_set"),
            (M::Port, "port"),
            (M::Final, "final"),
            #[cfg(not(target_os = "android"))]
            (M::ProcessName, "process_name"),
            #[cfg(target_os = "android")]
            (M::AppPackage, "app_package"),
        ];
        for (match_type, expected) in cases {
            let model = sample_rule(match_type);
            let view = LocalRuleView::from_model(&model);
            assert_eq!(
                view.match_type, expected,
                "match_type {:?}",
                model.match_type
            );
            let back = rule_round_trip(model.clone());
            assert_eq!(
                back, model,
                "match_type {:?} must survive round trip",
                model.match_type
            );
        }
    }

    #[test]
    fn every_action_round_trips_through_view_string() {
        use pp_client::local_override::RuleAction as A;
        let cases: Vec<(A, &str)> = vec![
            (A::Proxy, "proxy"),
            (A::Direct, "direct"),
            (A::Reject, "reject"),
            (
                A::Outbound {
                    tag: "my-group".to_string(),
                },
                "outbound:my-group",
            ),
        ];
        for (action, expected) in cases {
            let mut model = sample_rule(pp_client::local_override::RuleMatchType::Domain);
            model.action = action;
            let view = LocalRuleView::from_model(&model);
            assert_eq!(view.action, expected, "action {:?}", model.action);
            let back = rule_round_trip(model.clone());
            assert_eq!(
                back, model,
                "action {:?} must survive round trip",
                model.action
            );
        }
    }

    #[test]
    fn rule_set_kind_round_trips_through_view_string() {
        use pp_client::local_override::{LocalRuleSetRef, RuleSetKind as K, RuleSetSource};
        let cases: Vec<(K, &str)> = vec![
            (K::SingBoxRemote, "singbox_remote"),
            (K::SingBoxLocal, "singbox_local"),
        ];
        for (kind, expected) in cases {
            let model = LocalRuleSetRef {
                id: "rs1".to_string(),
                name: "RS".to_string(),
                tag: "rs-tag".to_string(),
                kind,
                source: RuleSetSource::Remote {
                    url: "https://example.com/x.srs".to_string(),
                },
                enabled: true,
                auto_update_interval_minutes: 0,
                last_updated: 0,
            };
            let view = LocalRuleSetRefView::from_model(&model);
            assert_eq!(view.kind, expected, "kind {:?}", model.kind);
            let input = LocalRuleSetRefInput {
                id: view.id,
                name: view.name,
                tag: view.tag,
                kind: view.kind,
                source: view.source,
                enabled: view.enabled,
                auto_update_interval_minutes: view.auto_update_interval_minutes,
                last_updated: view.last_updated,
            };
            let back = convert_rule_set_ref_input(input).unwrap();
            assert_eq!(back.kind, model.kind);
        }
    }
}
