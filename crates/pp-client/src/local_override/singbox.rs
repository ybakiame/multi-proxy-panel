//! sing-box local override injection.
//!
//! ADR-0002, section 3.1.3 and 3.4.3.
//!
//! Injection point: after `compose_singbox_config`, before `apply_panel_features`.
//!
//! Strategy:
//! 1. `rule_sets`: append to `route.rule_sets` (remote rule_set array).
//! 2. `rules`: prepend to `route.rules` array head (local rules take priority over subscription rules).
//! 3. `final`: if a Final-type rule exists, write to `route.final`.

use serde_json::{Value, json};

use super::{CoreLocalOverride, LocalRule, RuleMatchType};

#[cfg(test)]
use super::RuleAction;

/// Apply local override to a composed sing-box config.
///
/// No-op if `ovr.enabled` is false.
pub fn apply_singbox_local_override(config: &mut Value, ovr: &CoreLocalOverride) {
    if !ovr.enabled {
        return;
    }
    let Some(obj) = config.as_object_mut() else {
        return;
    };

    // 1. Build and inject rule_sets.
    inject_singbox_rule_sets(obj, ovr);

    // 2. Build and inject rules.
    inject_singbox_rules(obj, ovr);

    // 3. Handle final rule.
    inject_singbox_final(obj, ovr);
}

// ---------------------------------------------------------------------------
// rule_sets injection
// ---------------------------------------------------------------------------

fn inject_singbox_rule_sets(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let rule_set_entries: Vec<Value> = ovr
        .rule_sets
        .iter()
        .filter(|rs| rs.enabled)
        .filter_map(build_singbox_rule_set_entry)
        .collect();

    if rule_set_entries.is_empty() {
        return;
    }

    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let rule_sets = route_obj
        .entry("rule_sets")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = rule_sets.as_array_mut() else {
        return;
    };
    for entry in rule_set_entries {
        arr.push(entry);
    }
}

fn build_singbox_rule_set_entry(rs: &super::LocalRuleSetRef) -> Option<Value> {
    let url = match &rs.source {
        super::RuleSetSource::Remote { url } => url.clone(),
        super::RuleSetSource::Local { path } => path.clone(),
        super::RuleSetSource::Bundled { name } => {
            // Bundled resources use the name as relative path.
            return Some(json!({
                "type": "local",
                "tag": rs.tag,
                "format": "source",
                "path": name,
            }));
        }
    };

    let format = match rs.kind {
        super::RuleSetKind::SingBoxRemote => "binary",
        super::RuleSetKind::SingBoxLocal => "source",
        _ => "binary",
    };

    Some(json!({
        "type": "remote",
        "tag": rs.tag,
        "format": format,
        "url": url,
        "download_detour": "proxy",
    }))
}

// ---------------------------------------------------------------------------
// rules injection
// ---------------------------------------------------------------------------

fn inject_singbox_rules(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let mut local_rules: Vec<Value> = ovr
        .rules
        .iter()
        .filter(|r| r.enabled)
        .filter(|r| !matches!(r.match_type, RuleMatchType::Final))
        .map(build_singbox_rule_entry)
        .collect();

    if local_rules.is_empty() && ovr.rule_sets.iter().filter(|rs| rs.enabled).count() == 0 {
        return;
    }

    // Append rule_set references as rules.
    for rs in &ovr.rule_sets {
        if !rs.enabled {
            continue;
        }
        local_rules.push(json!({
            "rule_set": rs.tag,
            "outbound": "proxy",
        }));
    }

    if local_rules.is_empty() {
        return;
    }

    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let existing_rules = route_obj
        .remove("rules")
        .and_then(|r| r.as_array().cloned())
        .unwrap_or_default();

    // Prepend local rules (higher priority than subscription rules).
    let mut combined = local_rules;
    combined.extend(existing_rules);
    route_obj.insert("rules".to_string(), Value::Array(combined));
}

fn build_singbox_rule_entry(rule: &LocalRule) -> Value {
    let mut map = serde_json::Map::new();

    match &rule.match_type {
        RuleMatchType::Domain => {
            map.insert("domain".to_string(), Value::String(rule.target.clone()));
        }
        RuleMatchType::DomainSuffix => {
            map.insert(
                "domain_suffix".to_string(),
                Value::String(rule.target.clone()),
            );
        }
        RuleMatchType::DomainKeyword => {
            map.insert(
                "domain_keyword".to_string(),
                Value::String(rule.target.clone()),
            );
        }
        RuleMatchType::IpCidr => {
            map.insert("ip_cidr".to_string(), Value::String(rule.target.clone()));
        }
        RuleMatchType::SourceIpCidr => {
            map.insert(
                "source_ip_cidr".to_string(),
                Value::String(rule.target.clone()),
            );
        }
        RuleMatchType::RuleSet => {
            map.insert("rule_set".to_string(), Value::String(rule.target.clone()));
        }
        #[cfg(target_os = "android")]
        RuleMatchType::AppPackage => {
            map.insert(
                "package_name".to_string(),
                Value::String(rule.target.clone()),
            );
        }
        #[cfg(not(target_os = "android"))]
        RuleMatchType::ProcessName => {
            map.insert(
                "process_name".to_string(),
                Value::String(rule.target.clone()),
            );
        }
        RuleMatchType::Port => {
            map.insert("port".to_string(), Value::String(rule.target.clone()));
        }
        RuleMatchType::Final => {
            // Final is handled separately via route.final.
        }
    }

    map.insert(
        "outbound".to_string(),
        Value::String(rule.action.outbound_tag().to_string()),
    );

    if rule.advanced.invert {
        map.insert("invert".to_string(), Value::Bool(true));
    }

    Value::Object(map)
}

// ---------------------------------------------------------------------------
// final injection
// ---------------------------------------------------------------------------

fn inject_singbox_final(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let Some(final_rule) = ovr
        .rules
        .iter()
        .find(|r| r.enabled && matches!(r.match_type, RuleMatchType::Final))
    else {
        return;
    };

    let route = obj
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    route_obj.insert(
        "final".to_string(),
        Value::String(final_rule.action.outbound_tag().to_string()),
    );
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
        let mut config = json!({"route": {"rules": [{"outbound": "proxy"}]}});
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
        apply_singbox_local_override(&mut config, &ovr);
        let rules = config["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn rules_prepended_before_subscription_rules() {
        let mut config = json!({
            "route": {
                "rules": [
                    {"domain": "sub.com", "outbound": "proxy"}
                ]
            }
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
        apply_singbox_local_override(&mut config, &ovr);
        let rules = config["route"]["rules"].as_array().unwrap();
        assert_eq!(rules.len(), 3);
        // Local rules first.
        assert_eq!(rules[0]["domain"], "example.com");
        assert_eq!(rules[1]["domain_suffix"], "google.com");
        // Subscription rule last.
        assert_eq!(rules[2]["domain"], "sub.com");
    }

    #[test]
    fn rule_sets_appended_to_route_rule_sets() {
        let mut config = json!({"route": {}});
        let ovr = CoreLocalOverride {
            enabled: true,
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
                last_updated: 0,
            }],
            ..Default::default()
        };
        apply_singbox_local_override(&mut config, &ovr);
        let rule_sets = config["route"]["rule_sets"].as_array().unwrap();
        assert_eq!(rule_sets.len(), 1);
        assert_eq!(rule_sets[0]["tag"], "geoip-cn");
        assert_eq!(rule_sets[0]["type"], "remote");
    }

    #[test]
    fn final_rule_writes_route_final() {
        let mut config = json!({"route": {"rules": []}});
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
        apply_singbox_local_override(&mut config, &ovr);
        assert_eq!(config["route"]["final"], "direct");
    }

    #[test]
    fn rule_order_local_cards_then_rule_sets_then_subscription() {
        let mut config = json!({
            "route": {
                "rules": [{"domain": "sub.com", "outbound": "proxy"}]
            }
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
                kind: RuleSetKind::SingBoxRemote,
                source: RuleSetSource::Remote {
                    url: "https://example.com/cn.srs".to_string(),
                },
                enabled: true,
                auto_update_interval_minutes: 0,
                last_updated: 0,
            }],
        };
        apply_singbox_local_override(&mut config, &ovr);
        let rules = config["route"]["rules"].as_array().unwrap();
        // [0] local card, [1] rule-set ref, [2] subscription rule.
        assert_eq!(rules.len(), 3);
        assert_eq!(rules[0]["domain"], "example.com");
        assert_eq!(rules[1]["rule_set"], "geoip-cn");
        assert_eq!(rules[2]["domain"], "sub.com");
    }

    #[test]
    fn all_match_types_translate_correctly() {
        let cases = vec![
            (RuleMatchType::Domain, "domain", "example.com"),
            (RuleMatchType::DomainSuffix, "domain_suffix", "example.com"),
            (RuleMatchType::DomainKeyword, "domain_keyword", "example"),
            (RuleMatchType::IpCidr, "ip_cidr", "192.168.0.0/16"),
            (RuleMatchType::SourceIpCidr, "source_ip_cidr", "10.0.0.0/8"),
            (RuleMatchType::RuleSet, "rule_set", "geoip-cn"),
            (RuleMatchType::Port, "port", "443"),
        ];

        for (match_type, expected_key, target) in cases {
            let mut config = json!({"route": {"rules": []}});
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
            apply_singbox_local_override(&mut config, &ovr);
            let rules = config["route"]["rules"].as_array().unwrap();
            assert_eq!(
                rules.len(),
                1,
                "match_type {:?} should produce a rule",
                match_type
            );
            assert!(
                rules[0].get(expected_key).is_some(),
                "match_type {:?} should have key {}",
                match_type,
                expected_key
            );
        }
    }

    #[test]
    fn invert_flag_translates() {
        let mut config = json!({"route": {"rules": []}});
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
                    invert: true,
                    ..Default::default()
                },
                note: String::new(),
                created_at: 0,
                sort_order: 0,
            }],
            ..Default::default()
        };
        apply_singbox_local_override(&mut config, &ovr);
        let rules = config["route"]["rules"].as_array().unwrap();
        assert_eq!(rules[0]["invert"], true);
    }
}
