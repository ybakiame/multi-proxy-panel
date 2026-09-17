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
    parse_rule_set_tags,
};

/// Apply local override to a composed sing-box config.
///
/// No-op if the config is not a JSON object. Rules / rule sets are injected per
/// their own `enabled` flag (the removed master switch is no longer consulted).
pub fn apply_singbox_local_override(config: &mut Value, ovr: &CoreLocalOverride) {
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
/// **referenced by at least one already-injected `rule_set` reference**, and
/// whose backing file exists on disk.
///
/// References are collected from two places (merged, deduplicated):
///
/// 1. Enabled rule cards with `match_type == rule_set` (route rules).
/// 2. Already-rendered DNS rules in `config.dns.rules[].rule_set`. The DNS slice
///    is applied before this layer (ADR-0005 §3.2 ⓪), so the rendered config
///    already carries these references; both the array and single-string forms
///    of the `rule_set` match field are accepted.
///
/// Reference-driven semantics (aligns with user expectations of the rule
/// editor):
///
/// - A custom rule set that no reference points at is **not** injected — merely
///   adding a rule set (without a rule card or DNS rule using its tag) must not
///   touch the generated config.
/// - Since the「移除 enabled」refactor rule sets are pure resources: no enabled
///   gate remains. A rule set referenced by an injected reference but with a
///   **missing backing file** is skipped. In that case the referencing rule
///   keeps a dangling tag and `sing-box check` fails with a clear
///   "rule set not found: <tag>"-style error — an explicit, user-perceivable
///   signal to update the rule set (we deliberately do not inject a half-broken
///   rule set entry).
///
/// Entry shape aligns with [`build_singbox_rule_set_entry`]'s local output:
/// `{ "type": "local", "tag", "format": "source"|"binary", "path" }`.
/// Only entries are appended — matching rules still come from user rule
/// cards / DNS rules referencing the custom tag.
pub fn apply_custom_rule_sets(
    config: &mut Value,
    manager: &RuleSetManager,
    rules: &[LocalRule],
    custom_sets: &[CustomRuleSet],
) {
    let Some(obj) = config.as_object_mut() else {
        return;
    };

    // Tags referenced by enabled `rule_set` rule cards (route rules). A card
    // target may carry multiple comma-separated tags; each referenced tag is
    // collected so every custom set it names is injected.
    let mut referenced_tags: HashSet<String> = rules
        .iter()
        .filter(|r| r.enabled && matches!(r.match_type, RuleMatchType::RuleSet))
        .flat_map(|r| parse_rule_set_tags(&r.target))
        .collect();

    // Plus tags referenced by already-rendered DNS rules. Merged into one set so
    // a tag referenced by both route and DNS yields exactly one entry.
    referenced_tags.extend(collect_dns_rule_set_tags(obj));

    if referenced_tags.is_empty() {
        return;
    }

    let entries: Vec<Value> = custom_sets
        .iter()
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

    // De-duplicate: one entry per tag in `route.rule_set`. A built-in rule set exists both as
    // the remote entry registered by the template and as a materialized custom entry, so the
    // user-downloaded custom file wins (local path replaces remote) and sing-box never sees a
    // duplicated tag.
    remove_rule_set_entries_by_tag(obj, &entries);
    append_route_rule_set_entries(obj, entries);
}

/// Remove existing `route.rule_set` entries sharing a tag with `entries` (making room for the
/// append that follows).
fn remove_rule_set_entries_by_tag(
    route_container: &mut serde_json::Map<String, Value>,
    entries: &[Value],
) {
    if entries.is_empty() {
        return;
    }
    let tags: Vec<String> = entries
        .iter()
        .filter_map(|entry| entry.get("tag").and_then(Value::as_str).map(String::from))
        .collect();
    let Some(arr) = route_container
        .get_mut("route")
        .and_then(Value::as_object_mut)
        .and_then(|route| route.get_mut("rule_set"))
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    arr.retain(|entry| {
        entry
            .get("tag")
            .and_then(Value::as_str)
            .is_none_or(|tag| !tags.iter().any(|t| t == tag))
    });
}

/// Collect `rule_set` tags referenced by already-rendered DNS rules
/// (`config.dns.rules[].rule_set`).
///
/// The sing-box `rule_set` match field accepts either a single string or an
/// array of strings; both forms are collected. Missing / malformed DNS rules
/// contribute nothing (matching the route-side tolerance for unknown tags).
fn collect_dns_rule_set_tags(obj: &serde_json::Map<String, Value>) -> HashSet<String> {
    obj.get("dns")
        .and_then(Value::as_object)
        .and_then(|dns| dns.get("rules"))
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| rule.get("rule_set"))
                .flat_map(rule_set_tags)
                .collect()
        })
        .unwrap_or_default()
}

/// Flatten a `rule_set` match field value into its tag strings (single string
/// or array of strings; anything else is ignored).
fn rule_set_tags(value: &Value) -> Vec<String> {
    match value {
        Value::String(tag) => vec![tag.clone()],
        Value::Array(tags) => tags
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        _ => Vec::new(),
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
    // 统一排序：内置规则与用户规则同处一个列表（物化模型，2026-09），渲染前显式按
    // （sort_order, created_at, id）排序，不再依赖存储顺序。
    let mut ordered: Vec<&LocalRule> = ovr.rules.iter().collect();
    ordered.sort_by(|a, b| {
        a.sort_order
            .cmp(&b.sort_order)
            .then(a.created_at.cmp(&b.created_at))
            .then(a.id.cmp(&b.id))
    });
    let mut local_rules: Vec<Value> = ordered
        .into_iter()
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
            // A single tag keeps the legacy string form (no snapshot diff for
            // existing data); two or more render as a string array, which
            // sing-box accepts for the `rule_set` match field.
            let tags = parse_rule_set_tags(&rule.target);
            let value = match tags.as_slice() {
                [only] => Value::String(only.clone()),
                _ => Value::Array(tags.iter().cloned().map(Value::String).collect()),
            };
            map.insert("rule_set".to_string(), value);
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
