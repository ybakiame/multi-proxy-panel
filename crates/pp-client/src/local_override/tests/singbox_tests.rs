//! sing-box local override injection tests (split out of `singbox.rs` to stay
//! within the business-file size gate; see `.agents/rules/code-organization.md`).

use super::*;
use crate::local_override::{LocalRuleSetRef, RuleAction, RuleSetKind, RuleSetSource};

fn sample_rule(id: &str, match_type: RuleMatchType, target: &str, action: RuleAction) -> LocalRule {
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
fn rule_sets_appended_to_route_rule_set() {
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
    // sing-box route field is singular `rule_set`; plural `rule_sets` is rejected by check.
    assert!(
        config["route"].get("rule_sets").is_none(),
        "no plural rule_sets key allowed"
    );
    let rule_sets = config["route"]["rule_set"].as_array().unwrap();
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

// -----------------------------------------------------------------------
// Custom rule set injection
// -----------------------------------------------------------------------

fn sample_custom_rule_set(
    id: &str,
    tag: &str,
    source: crate::local_override::CustomRuleSetSource,
) -> super::CustomRuleSet {
    super::CustomRuleSet {
        id: id.to_string(),
        name: String::new(),
        tag: tag.to_string(),
        source,
        last_updated: 0,
        remote_updated_at: 0,
    }
}

fn rule_set_entries(config: &serde_json::Value) -> Vec<&serde_json::Value> {
    config["route"]["rule_set"]
        .as_array()
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

/// An enabled rule card referencing the given custom tag via `match_type == rule_set`.
fn referencing_rule(tag: &str) -> LocalRule {
    LocalRule {
        id: format!("rule-{tag}"),
        name: String::new(),
        enabled: true,
        match_type: RuleMatchType::RuleSet,
        target: tag.to_string(),
        action: RuleAction::Proxy,
        advanced: Default::default(),
        note: String::new(),
        created_at: 0,
        sort_order: 0,
    }
}

#[test]
fn custom_manual_rule_set_injected_as_local_source_when_file_exists_and_referenced() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let rs = sample_custom_rule_set(
        "c1",
        "manual-custom",
        crate::local_override::CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
    );
    let rules = [referencing_rule("manual-custom")];

    // No backing file yet → no entry.
    let mut config = json!({"route": {}});
    apply_custom_rule_sets(&mut config, &mgr, &rules, std::slice::from_ref(&rs));
    assert!(rule_set_entries(&config).is_empty());

    // Backing file exists (written at save) → local source entry under `route.rule_set`.
    let manual_path = mgr.custom_rule_set_file_path(&rs.id, super::RuleSetFormat::Source);
    std::fs::create_dir_all(manual_path.parent().unwrap()).unwrap();
    std::fs::write(manual_path, "[]").unwrap();
    let mut config = json!({"route": {}});
    apply_custom_rule_sets(&mut config, &mgr, &rules, &[rs]);
    assert!(
        config["route"].get("rule_sets").is_none(),
        "no plural rule_sets key allowed"
    );
    let entries = rule_set_entries(&config);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["type"], "local");
    assert_eq!(entries[0]["tag"], "manual-custom");
    assert_eq!(entries[0]["format"], "source");
    assert!(
        entries[0]["path"].as_str().unwrap().ends_with("c1.json"),
        "unexpected path: {}",
        entries[0]["path"]
    );
}

#[test]
fn custom_rule_set_not_injected_when_no_rule_references_it() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let rs = sample_custom_rule_set(
        "c1",
        "manual-custom",
        crate::local_override::CustomRuleSetSource::Manual {
            content: "[]".to_string(),
        },
    );
    let manual_path = mgr.custom_rule_set_file_path(&rs.id, super::RuleSetFormat::Source);
    std::fs::create_dir_all(manual_path.parent().unwrap()).unwrap();
    std::fs::write(manual_path, "[]").unwrap();

    // Backing file present but referenced by a *different* tag (or by nothing).
    let mut config = json!({"route": {}});
    apply_custom_rule_sets(
        &mut config,
        &mgr,
        &[referencing_rule("other-tag")],
        std::slice::from_ref(&rs),
    );
    assert!(
        rule_set_entries(&config).is_empty(),
        "unreferenced rule set must not be injected"
    );

    let mut config = json!({"route": {}});
    let empty_rules: [LocalRule; 0] = [];
    apply_custom_rule_sets(&mut config, &mgr, &empty_rules, &[rs]);
    assert!(
        rule_set_entries(&config).is_empty(),
        "no rule at all → nothing injected"
    );
}

#[test]
fn custom_remote_rule_set_injected_only_when_cached_and_referenced() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let mk = || {
        sample_custom_rule_set(
            "r1",
            "remote-custom",
            crate::local_override::CustomRuleSetSource::Remote {
                url: "https://example.com/x.srs".to_string(),
                format: super::RuleSetFormat::Binary,
            },
        )
    };
    let rules = [referencing_rule("remote-custom")];

    // Referenced but not cached → skipped (no enabled gate anymore).
    let mut config = json!({});
    apply_custom_rule_sets(&mut config, &mgr, &rules, &[mk()]);
    assert!(rule_set_entries(&config).is_empty());

    // Cached + referenced → local binary entry under `route.rule_set`.
    let cache = mgr.custom_rule_set_file_path("r1", super::RuleSetFormat::Binary);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, "fake-srs").unwrap();
    let mut config = json!({});
    apply_custom_rule_sets(&mut config, &mgr, &rules, &[mk()]);
    assert!(
        config["route"].get("rule_sets").is_none(),
        "no plural rule_sets key allowed"
    );
    let entries = rule_set_entries(&config);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["type"], "local");
    assert_eq!(entries[0]["tag"], "remote-custom");
    assert_eq!(entries[0]["format"], "binary");
    assert!(entries[0]["path"].as_str().unwrap().ends_with("r1.srs"));
}

#[test]
fn custom_remote_source_format_injected_with_source_format() {
    let dir = tempfile::tempdir().unwrap();
    let mgr = RuleSetManager::new(dir.path().to_path_buf());
    let rs = sample_custom_rule_set(
        "r2",
        "remote-source",
        crate::local_override::CustomRuleSetSource::Remote {
            url: "https://example.com/x.json".to_string(),
            format: super::RuleSetFormat::Source,
        },
    );
    let cache = mgr.custom_rule_set_file_path("r2", super::RuleSetFormat::Source);
    std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
    std::fs::write(&cache, "[]").unwrap();

    let mut config = json!({});
    apply_custom_rule_sets(
        &mut config,
        &mgr,
        &[referencing_rule("remote-source")],
        &[rs],
    );
    let entries = rule_set_entries(&config);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["format"], "source");
    assert!(entries[0]["path"].as_str().unwrap().ends_with("r2.json"));
}
