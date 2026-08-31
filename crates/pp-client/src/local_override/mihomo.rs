//! mihomo local override injection.
//!
//! ADR-0002, section 3.1.3 and 3.4.3.
//!
//! Injection point: after `compose_mihomo_config`, before `apply_panel_features`.
//!
//! Strategy:
//! 1. `rule-providers`: write to top-level `rule-providers` mapping.
//! 2. `rules`: prepend to `rules` array head; rule-set references generate `RULE-SET,<tag>,<action>` format.
//! 3. `MATCH`: if a Final-type rule exists, replace the last `MATCH,proxy` or append.

use serde_json::{Value, json};

use super::{CoreLocalOverride, LocalRule, RuleMatchType};

#[cfg(test)]
use super::RuleAction;

/// Apply local override to a composed mihomo config.
///
/// No-op if `ovr.enabled` is false.
pub fn apply_mihomo_local_override(config: &mut Value, ovr: &CoreLocalOverride) {
    if !ovr.enabled {
        return;
    }
    let Some(obj) = config.as_object_mut() else {
        return;
    };

    // 1. Build and inject rule-providers.
    inject_mihomo_rule_providers(obj, ovr);

    // 2. Build and inject rules.
    inject_mihomo_rules(obj, ovr);

    // 3. Handle MATCH / final rule.
    inject_mihomo_match(obj, ovr);
}

// ---------------------------------------------------------------------------
// rule-providers injection
// ---------------------------------------------------------------------------

fn inject_mihomo_rule_providers(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let providers: serde_json::Map<String, Value> = ovr
        .rule_sets
        .iter()
        .filter(|rs| rs.enabled)
        .filter_map(build_mihomo_rule_provider)
        .collect();

    if providers.is_empty() {
        return;
    }

    let existing = obj
        .entry("rule-providers")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(existing_map) = existing.as_object_mut() else {
        return;
    };
    for (k, v) in providers {
        existing_map.insert(k, v);
    }
}

fn build_mihomo_rule_provider(rs: &super::LocalRuleSetRef) -> Option<(String, Value)> {
    let (behavior, path) = infer_mihomo_behavior(&rs.tag);

    let url = match &rs.source {
        super::RuleSetSource::Remote { url } => url.clone(),
        super::RuleSetSource::Local { path } => path.clone(),
        super::RuleSetSource::Bundled { name } => name.clone(),
    };

    let provider = json!({
        "type": "http",
        "behavior": behavior,
        "url": url,
        "path": path,
        "interval": rs.auto_update_interval_minutes.max(1) * 60,
    });

    Some((rs.tag.clone(), provider))
}

/// Infer mihomo rule-provider behavior from tag name heuristic.
///
/// Returns `(behavior, suggested_path)`.
fn infer_mihomo_behavior(tag: &str) -> (&'static str, String) {
    if tag.starts_with("geoip") {
        ("ipcidr", format!("./rule_sets/{tag}.yaml"))
    } else if tag.starts_with("geosite") || tag.contains("ads") {
        ("domain", format!("./rule_sets/{tag}.yaml"))
    } else {
        ("classical", format!("./rule_sets/{tag}.yaml"))
    }
}

// ---------------------------------------------------------------------------
// rules injection
// ---------------------------------------------------------------------------

fn inject_mihomo_rules(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let mut local_rules: Vec<Value> = ovr
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter(|r| !matches!(r.match_type, RuleMatchType::Final))
        .map(build_mihomo_rule_string)
        .map(Value::String)
        .collect();

    // Append rule-set references as RULE-SET strings.
    for rs in &ovr.rule_sets {
        if !rs.enabled {
            continue;
        }
        // Default action for rule-set references is proxy; user can override via local rules.
        local_rules.push(Value::String(format!("RULE-SET,{},proxy", rs.tag)));
    }

    if local_rules.is_empty() {
        return;
    }

    let existing_rules = obj
        .remove("rules")
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();

    // Prepend local rules (higher priority).
    let mut combined = local_rules;
    combined.extend(existing_rules);
    obj.insert("rules".to_string(), Value::Array(combined));
}

fn build_mihomo_rule_string(rule: &LocalRule) -> String {
    let prefix = match &rule.match_type {
        RuleMatchType::Domain => "DOMAIN",
        RuleMatchType::DomainSuffix => "DOMAIN-SUFFIX",
        RuleMatchType::DomainKeyword => "DOMAIN-KEYWORD",
        RuleMatchType::IpCidr => "IP-CIDR",
        RuleMatchType::SourceIpCidr => "SRC-IP-CIDR",
        RuleMatchType::RuleSet => "RULE-SET",
        #[cfg(target_os = "android")]
        RuleMatchType::AppPackage => "PROCESS-NAME",
        #[cfg(not(target_os = "android"))]
        RuleMatchType::ProcessName => "PROCESS-NAME",
        RuleMatchType::Port => "DST-PORT",
        RuleMatchType::Final => "MATCH",
    };

    let action = rule.action.outbound_tag();
    let mut s = format!("{},{},{}", prefix, rule.target, action);
    if rule.advanced.no_resolve && !matches!(rule.match_type, RuleMatchType::Final) {
        s.push_str(",no-resolve");
    }
    s
}

// ---------------------------------------------------------------------------
// MATCH / final injection
// ---------------------------------------------------------------------------

fn inject_mihomo_match(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let Some(final_rule) = ovr
        .rules
        .iter()
        .find(|r| r.enabled && matches!(r.match_type, RuleMatchType::Final))
    else {
        return;
    };

    let action = final_rule.action.outbound_tag();
    let match_str = format!("MATCH,{action}");

    let mut rules = obj
        .remove("rules")
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();

    // Try to replace existing MATCH rule.
    let mut replaced = false;
    for rule in &mut rules {
        if let Some(s) = rule.as_str()
            && s.starts_with("MATCH,")
        {
            *rule = Value::String(match_str.clone());
            replaced = true;
            break;
        }
    }

    if !replaced {
        rules.push(Value::String(match_str));
    }

    obj.insert("rules".to_string(), Value::Array(rules));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_override::{LocalRuleSetRef, RuleSetKind, RuleSetSource};

    fn sample_rule(
        id: &str,
        match_type: RuleMatchType,
        target: &str,
        action: RuleAction,
    ) -> LocalRule {
        LocalRule {
            id: id.to_string(),
            name: String::new(),
            enabled: true,
            match_type,
            target: target.to_string(),
            action,
            advanced: Default::default(),
            note: String::new(),
            created_at: 0,
            sort_order: 0,
        }
    }

    #[test]
    fn inject_disabled_is_noop() {
        let mut config = json!({"rules": ["DOMAIN,sub.com,proxy"]});
        let ovr = CoreLocalOverride {
            enabled: false,
            rules: vec![sample_rule(
                "r1",
                RuleMatchType::Domain,
                "example.com",
                RuleAction::Direct,
            )],
            ..Default::default()
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let rules = config["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn rules_prepended_before_subscription_rules() {
        let mut config = json!({
            "rules": ["DOMAIN,sub.com,proxy"]
        });
        let ovr = CoreLocalOverride {
            enabled: true,
            rules: vec![
                sample_rule(
                    "r1",
                    RuleMatchType::Domain,
                    "example.com",
                    RuleAction::Direct,
                ),
                sample_rule(
                    "r2",
                    RuleMatchType::DomainSuffix,
                    "google.com",
                    RuleAction::Proxy,
                ),
            ],
            ..Default::default()
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let rules = config["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].as_str().unwrap(), "DOMAIN,example.com,direct");
        assert_eq!(rules[1].as_str().unwrap(), "DOMAIN-SUFFIX,google.com,proxy");
        assert_eq!(rules[2].as_str().unwrap(), "DOMAIN,sub.com,proxy");
    }

    #[test]
    fn rule_providers_written_to_top_level() {
        let mut config = json!({});
        let ovr = CoreLocalOverride {
            enabled: true,
            rule_sets: vec![LocalRuleSetRef {
                id: "rs1".to_string(),
                name: "GeoIP CN".to_string(),
                tag: "geoip-cn".to_string(),
                kind: RuleSetKind::MihomoHttp,
                source: RuleSetSource::Remote {
                    url: "https://example.com/cn.yaml".to_string(),
                },
                enabled: true,
                auto_update_interval_minutes: 1440,
                last_updated: 0,
            }],
            ..Default::default()
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let providers = config["rule-providers"].as_object().unwrap();
        assert!(providers.contains_key("geoip-cn"));
        assert_eq!(providers["geoip-cn"]["type"], "http");
        assert_eq!(providers["geoip-cn"]["behavior"], "ipcidr");
    }

    #[test]
    fn final_rule_replaces_match() {
        let mut config = json!({
            "rules": ["DOMAIN,sub.com,proxy", "MATCH,proxy"]
        });
        let ovr = CoreLocalOverride {
            enabled: true,
            rules: vec![LocalRule {
                id: "final1".to_string(),
                name: "final".to_string(),
                enabled: true,
                match_type: RuleMatchType::Final,
                target: String::new(),
                action: RuleAction::Direct,
                advanced: Default::default(),
                note: String::new(),
                created_at: 0,
                sort_order: 999,
            }],
            ..Default::default()
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let rules = config["rules"].as_array().unwrap();
        assert_eq!(rules.last().unwrap().as_str().unwrap(), "MATCH,direct");
    }

    #[test]
    fn final_rule_appends_when_no_existing_match() {
        let mut config = json!({
            "rules": ["DOMAIN,sub.com,proxy"]
        });
        let ovr = CoreLocalOverride {
            enabled: true,
            rules: vec![LocalRule {
                id: "final1".to_string(),
                name: "final".to_string(),
                enabled: true,
                match_type: RuleMatchType::Final,
                target: String::new(),
                action: RuleAction::Reject,
                advanced: Default::default(),
                note: String::new(),
                created_at: 0,
                sort_order: 999,
            }],
            ..Default::default()
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let rules = config["rules"].as_array().unwrap();
        assert_eq!(rules.last().unwrap().as_str().unwrap(), "MATCH,reject");
    }

    #[test]
    fn rule_order_local_cards_then_rule_sets_then_subscription() {
        let mut config = json!({
            "rules": ["DOMAIN,sub.com,proxy"]
        });
        let ovr = CoreLocalOverride {
            enabled: true,
            rules: vec![sample_rule(
                "r1",
                RuleMatchType::Domain,
                "example.com",
                RuleAction::Direct,
            )],
            rule_sets: vec![LocalRuleSetRef {
                id: "rs1".to_string(),
                name: "GeoIP CN".to_string(),
                tag: "geoip-cn".to_string(),
                kind: RuleSetKind::MihomoHttp,
                source: RuleSetSource::Remote {
                    url: "https://example.com/cn.yaml".to_string(),
                },
                enabled: true,
                auto_update_interval_minutes: 0,
                last_updated: 0,
            }],
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let rules = config["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0].as_str().unwrap(), "DOMAIN,example.com,direct");
        assert_eq!(rules[1].as_str().unwrap(), "RULE-SET,geoip-cn,proxy");
        assert_eq!(rules[2].as_str().unwrap(), "DOMAIN,sub.com,proxy");
    }

    #[test]
    fn all_match_types_translate_correctly() {
        let cases = vec![
            (RuleMatchType::Domain, "DOMAIN", "example.com"),
            (RuleMatchType::DomainSuffix, "DOMAIN-SUFFIX", "example.com"),
            (RuleMatchType::DomainKeyword, "DOMAIN-KEYWORD", "example"),
            (RuleMatchType::IpCidr, "IP-CIDR", "192.168.0.0/16"),
            (RuleMatchType::SourceIpCidr, "SRC-IP-CIDR", "10.0.0.0/8"),
            (RuleMatchType::RuleSet, "RULE-SET", "geoip-cn"),
            (RuleMatchType::Port, "DST-PORT", "443"),
        ];

        for (match_type, expected_prefix, target) in cases {
            let mut config = json!({"rules": []});
            let ovr = CoreLocalOverride {
                enabled: true,
                rules: vec![sample_rule(
                    "r1",
                    match_type.clone(),
                    target,
                    RuleAction::Proxy,
                )],
                ..Default::default()
            };
            apply_mihomo_local_override(&mut config, &ovr);
            let rules = config["rules"].as_array().unwrap();
            assert_eq!(
                rules.len(),
                1,
                "match_type {:?} should produce a rule",
                match_type
            );
            let s = rules[0].as_str().unwrap();
            assert!(
                s.starts_with(expected_prefix),
                "match_type {:?} should start with {}, got {}",
                match_type,
                expected_prefix,
                s
            );
        }
    }

    #[test]
    fn no_resolve_flag_appended() {
        let mut config = json!({"rules": []});
        let ovr = CoreLocalOverride {
            enabled: true,
            rules: vec![LocalRule {
                id: "r1".to_string(),
                name: String::new(),
                enabled: true,
                match_type: RuleMatchType::Domain,
                target: "example.com".to_string(),
                action: RuleAction::Direct,
                advanced: crate::local_override::RuleAdvancedOptions {
                    no_resolve: true,
                    ..Default::default()
                },
                note: String::new(),
                created_at: 0,
                sort_order: 0,
            }],
            ..Default::default()
        };
        apply_mihomo_local_override(&mut config, &ovr);
        let rules = config["rules"].as_array().unwrap();
        assert_eq!(
            rules[0].as_str().unwrap(),
            "DOMAIN,example.com,direct,no-resolve"
        );
    }

    #[test]
    fn infer_behavior_geoip_returns_ipcidr() {
        let (behavior, path) = infer_mihomo_behavior("geoip-cn");
        assert_eq!(behavior, "ipcidr");
        assert_eq!(path, "./rule_sets/geoip-cn.yaml");
    }

    #[test]
    fn infer_behavior_geosite_returns_domain() {
        let (behavior, path) = infer_mihomo_behavior("geosite-ads");
        assert_eq!(behavior, "domain");
        assert_eq!(path, "./rule_sets/geosite-ads.yaml");
    }
}
