//! Conversion and validation helpers for local override commands.

use pp_client::local_override::{
    AppliedTemplate, CoreLocalOverride, LocalOverride, LocalRule, RuleSetManager,
    RuleSetSubscription,
};

use super::views::*;

/// Validate local override before saving.
///
/// Checks:
/// - Rule IDs are unique within each core.
/// - Targets are non-empty for non-Final rules.
/// - RuleSet references point to subscribed rule sets.
pub(super) fn validate_local_override(ovr: &LocalOverride) -> Result<(), String> {
    for (core_name, core_ovr) in [("singbox", &ovr.singbox), ("mihomo", &ovr.mihomo)] {
        // Check rule ID uniqueness.
        let mut seen = std::collections::HashSet::new();
        for rule in &core_ovr.rules {
            if !seen.insert(rule.id.clone()) {
                return Err(format!(
                    "duplicate rule id '{}' in {core_name}",
                    rule.id
                ));
            }
        }

        // Check non-empty targets.
        for rule in &core_ovr.rules {
            if !matches!(rule.match_type, pp_client::local_override::RuleMatchType::Final)
                && rule.target.trim().is_empty()
            {
                return Err(format!(
                    "rule '{}' in {core_name} has empty target",
                    rule.id
                ));
            }
        }

        // Check RuleSet references are subscribed.
        for rule in &core_ovr.rules {
            if matches!(rule.match_type, pp_client::local_override::RuleMatchType::RuleSet) {
                let subscribed = ovr
                    .rule_set_subscriptions
                    .iter()
                    .any(|s| s.community_id == rule.target && s.subscribed);
                if !subscribed {
                    return Err(format!(
                        "rule '{}' references unsubscribed rule set '{}'",
                        rule.id, rule.target
                    ));
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(super) fn convert_input_to_model(input: SaveLocalOverrideInput) -> Result<LocalOverride, String> {
    Ok(LocalOverride {
        singbox: convert_core_input(input.singbox)?,
        mihomo: convert_core_input(input.mihomo)?,
        rule_set_subscriptions: input
            .rule_set_subscriptions
            .into_iter()
            .map(convert_subscription_input)
            .collect(),
        applied_templates: input
            .applied_templates
            .into_iter()
            .map(convert_applied_template_input)
            .collect(),
    })
}

pub(super) fn convert_core_input(input: CoreLocalOverrideInput) -> Result<CoreLocalOverride, String> {
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
    let source = pp_client::local_override::RuleSetSource::Remote {
        url: input.source,
    };
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
        "mihomo_http" => Ok(pp_client::local_override::RuleSetKind::MihomoHttp),
        "mihomo_file" => Ok(pp_client::local_override::RuleSetKind::MihomoFile),
        _ => Err(format!("unknown rule_set kind: {s}")),
    }
}

fn convert_subscription_input(input: RuleSetSubscriptionInput) -> RuleSetSubscription {
    RuleSetSubscription {
        id: input.id,
        community_id: input.community_id,
        display_name: input.display_name,
        category: parse_rule_set_category(&input.category),
        subscribed: input.subscribed,
        singbox_url_template: input.singbox_url_template,
        mihomo_url_template: input.mihomo_url_template,
        default_interval_minutes: input.default_interval_minutes,
    }
}

fn parse_rule_set_category(s: &str) -> pp_client::local_override::RuleSetCategory {
    match s {
        "geoip" => pp_client::local_override::RuleSetCategory::Geoip,
        "geosite" => pp_client::local_override::RuleSetCategory::Geosite,
        "ads" => pp_client::local_override::RuleSetCategory::Ads,
        "privacy" => pp_client::local_override::RuleSetCategory::Privacy,
        "malware" => pp_client::local_override::RuleSetCategory::Malware,
        _ => pp_client::local_override::RuleSetCategory::Custom,
    }
}

fn convert_applied_template_input(input: AppliedTemplateInput) -> AppliedTemplate {
    AppliedTemplate {
        template_id: input.template_id,
        applied_at: input.applied_at,
        generated_rule_ids: input.generated_rule_ids,
    }
}

impl RuleSetStatusView {
    pub(crate) fn from_subscription(
        sub: &pp_client::local_override::RuleSetSubscription,
        manager: &RuleSetManager,
    ) -> Self {
        Self {
            id: sub.id.clone(),
            community_id: sub.community_id.clone(),
            display_name: sub.display_name.clone(),
            category: format!("{:?}", sub.category).to_lowercase(),
            subscribed: sub.subscribed,
            singbox_cached: manager.is_cached(&sub.community_id, pp_common::CoreType::SingBox),
            mihomo_cached: manager.is_cached(&sub.community_id, pp_common::CoreType::Mihomo),
            last_updated: 0,
        }
    }
}
