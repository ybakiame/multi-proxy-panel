//! sing-box local override injection.
//!
//! ADR-0002, section 3.1.3 and 3.4.3.
//!
//! Injection point: after `compose_singbox_config`, before `apply_panel_features`.
//!
//! Strategy:
//! 1. `rule_set`: append to `route.rule_set` (sing-box rule-set definitions
//!    array — note the key is **singular** `rule_set`, matching the sing-box
//!    JSON schema; a plural `rule_sets` key is rejected by `sing-box check`).
//! 2. `rules`: prepend to `route.rules` array head (local rules take priority over subscription rules).
//! 3. `final`: if a Final-type rule exists, write to `route.final`.

use std::collections::HashSet;

use serde_json::{Value, json};

use super::{
    CoreLocalOverride, CustomRuleSet, LocalRule, RuleMatchType, RuleSetFormat, RuleSetManager,
};

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
// rule_set injection
// ---------------------------------------------------------------------------

fn inject_singbox_rule_sets(obj: &mut serde_json::Map<String, Value>, ovr: &CoreLocalOverride) {
    let rule_set_entries: Vec<Value> = ovr
        .rule_sets
        .iter()
        .filter(|rs| rs.enabled)
        .filter_map(build_singbox_rule_set_entry)
        .collect();

    append_route_rule_set_entries(obj, rule_set_entries);
}

/// Append prebuilt rule_set definition entries into `route.rule_set`, creating
/// the `route` / `rule_set` nodes when missing. No-op when `entries` is empty
/// or the config is not an object.
///
/// Note the JSON key is the sing-box route field **`rule_set`** (singular);
/// `route.rule_sets` is not a valid sing-box field and fails `check`.
fn append_route_rule_set_entries(
    route_container: &mut serde_json::Map<String, Value>,
    entries: Vec<Value>,
) {
    if entries.is_empty() {
        return;
    }
    let route = route_container
        .entry("route")
        .or_insert_with(|| Value::Object(Default::default()));
    let Some(route_obj) = route.as_object_mut() else {
        return;
    };
    let rule_set = route_obj
        .entry("rule_set")
        .or_insert_with(|| Value::Array(Vec::new()));
    let Some(arr) = rule_set.as_array_mut() else {
        return;
    };
    arr.extend(entries);
}

/// Inject `type: local` rule_set entries for custom rule sets that are
/// **referenced by at least one enabled `rule_set` rule**, enabled themselves,
/// and whose backing file exists on disk.
///
/// Reference-driven semantics (aligns with user expectations of the rule
/// editor):
///
/// - A custom rule set that no enabled `rule_set` rule references is **not**
///   injected — merely adding a rule set (without a rule card using its tag)
///   must not touch the generated config.
/// - A rule set referenced by an enabled rule but **disabled** or with a
///   **missing backing file** is also skipped. In that case the referencing
///   rule keeps a dangling tag and `sing-box check` fails with a clear
///   "rule set not found: <tag>"-style error — an explicit, user-perceivable
///   signal to enable / update the rule set (we deliberately do not inject a
///   half-broken rule set entry).
///
/// Entry shape aligns with [`build_singbox_rule_set_entry`]'s local output:
/// `{ "type": "local", "tag", "format": "source"|"binary", "path" }`.
/// Only entries are appended — matching rules still come from user rule
/// cards referencing the custom tag.
pub fn apply_custom_rule_sets(
    config: &mut Value,
    manager: &RuleSetManager,
    rules: &[LocalRule],
    custom_sets: &[CustomRuleSet],
) {
    let Some(obj) = config.as_object_mut() else {
        return;
    };

    // Tags referenced by enabled `rule_set` rules (the only references the
    // generated config's rules actually use).
    let referenced_tags: HashSet<&str> = rules
        .iter()
        .filter(|r| r.enabled && matches!(r.match_type, RuleMatchType::RuleSet))
        .map(|r| r.target.as_str())
        .collect();
    if referenced_tags.is_empty() {
        return;
    }

    let entries: Vec<Value> = custom_sets
        .iter()
        .filter(|rs| rs.enabled)
        .filter(|rs| referenced_tags.contains(rs.tag.as_str()))
        .filter_map(|rs| {
            let format = rs.file_format();
            let path = manager.custom_rule_set_file_path(&rs.id, format);
            if !path.exists() {
                tracing::debug!(
                    id = %rs.id,
                    tag = %rs.tag,
                    path = %path.display(),
                    "custom rule set backing file missing, skipping injection"
                );
                return None;
            }
            let format_str = match format {
                RuleSetFormat::Source => "source",
                RuleSetFormat::Binary => "binary",
            };
            Some(json!({
                "type": "local",
                "tag": rs.tag,
                "format": format_str,
                "path": path.to_string_lossy(),
            }))
        })
        .collect();

    append_route_rule_set_entries(obj, entries);
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
#[path = "tests/singbox_tests.rs"]
mod tests;
