//! Local Override storage: reads/writes `data_dir/local_override.json`.
//!
//! Follows the same resilience pattern as [`ProfileStoreV2`]:
//! - Missing file → default (empty) config.
//! - Corrupted file → log warning, fall back to default (non-blocking).
//! - `#[serde(default)]` on all fields for forward compatibility.
//!
//! 自「废弃内置规则集订阅」起，`load` 内嵌**幂等存量迁移**：把旧版已订阅内置条目
//! 转换为用户自控的 custom Remote 规则集，使旧文件一次读取即归一化到纯用户自控
//! 模型。
//!
//! 自「移除场景模板」起，`load` 额外清空旧文件中的模板字段
//! （`applied_templates` / `custom_templates`）：字段仅保留 serde 兼容，写回空数组。
//!
//! 自「移除规则总开关」起，`load` 把旧文件中的 `singbox.enabled == false` 折叠为
//! 「所有规则卡片与规则集引用 `enabled = false`」（见
//! [`migrate_disabled_master_switch`]）；写回后该字段不再序列化，幂等。

use std::path::PathBuf;

use pp_common::PanelResult;
use serde_json::Value;

use super::{
    CustomRuleSet, CustomRuleSetSource, LocalOverride, LocalRule, RuleAction, RuleMatchType,
    RuleSetFormat,
};

/// Storage for `LocalOverride` at `data_dir/local_override.json`.
#[derive(Debug, Clone)]
pub struct LocalOverrideStore {
    data_dir: PathBuf,
}

impl LocalOverrideStore {
    /// Create storage based on data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/local_override.json`.
    pub fn override_file(&self) -> PathBuf {
        self.data_dir.join("local_override.json")
    }

    /// Load local override config.
    ///
    /// - File missing → returns default (empty) config.
    /// - File corrupted → logs warning, returns default (does not block startup).
    /// - Legacy file → runs the idempotent built-in mechanism migration, folds the
    ///   removed `singbox.enabled` master switch into per-rule / per-rule-set
    ///   switches, and clears the removed scenario-template fields, then
    ///   best-effort persists the normalized state back to disk.
    pub fn load(&self) -> PanelResult<LocalOverride> {
        let path = self.override_file();
        if !path.exists() {
            // Missing file: seed the built-in rule (2026-09: the built-in split rule is
            // materialized into the unified rule list) and persist so later loads see the
            // normalized document.
            let mut ovr = LocalOverride::default();
            seed_builtin_route_rules(&mut ovr);
            seed_builtin_rule_sets(&mut ovr);
            ovr.builtins_seeded = true;
            if let Err(e) = self.save(&ovr) {
                tracing::warn!(error = %e, "failed to persist builtin rule seeding");
            }
            return Ok(ovr);
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local_override.json unreadable, fall back to default"
                );
                return Ok(LocalOverride::default());
            }
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            tracing::warn!(
                path = %path.display(),
                "local_override.json corrupted, fall back to seeded default"
            );
            // Corrupted-file fallback seeds the built-ins too (same semantics as the
            // missing-file path).
            let mut ovr = LocalOverride::default();
            seed_builtin_route_rules(&mut ovr);
            seed_builtin_rule_sets(&mut ovr);
            ovr.builtins_seeded = true;
            return Ok(ovr);
        };
        let mut ovr: LocalOverride = match serde_json::from_value(value.clone()) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local_override.json schema mismatch, fall back to seeded default"
                );
                let mut ovr = LocalOverride::default();
                seed_builtin_route_rules(&mut ovr);
                seed_builtin_rule_sets(&mut ovr);
                ovr.builtins_seeded = true;
                return Ok(ovr);
            }
        };
        let migrated_builtins = migrate_legacy_builtins(&mut ovr);
        let migrated_switch = migrate_disabled_master_switch(&value, &mut ovr);
        let migrated_retired = retire_legacy_builtin_references(&mut ovr);
        // Built-ins are materialized once per file; afterwards the flag keeps user deletions
        // intact and only the `builtin` marker is re-normalized.
        let seeded = if ovr.builtins_seeded {
            let mut seeded = normalize_builtin_route_rules(&mut ovr);
            seeded |= normalize_builtin_rule_sets(&mut ovr);
            seeded
        } else {
            seed_builtin_route_rules(&mut ovr);
            seed_builtin_rule_sets(&mut ovr);
            ovr.builtins_seeded = true;
            true
        };
        if migrated_builtins || migrated_switch || migrated_retired || seeded {
            // Persist the migration result so later loads read normalized data (idempotent).
            if let Err(e) = self.save(&ovr) {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to persist legacy migration result"
                );
            }
        }
        Ok(ovr)
    }

    /// Save local override config to `data_dir/local_override.json`.
    ///
    /// Built-in rule protection (2026-09): a built-in rule is an ordinary rule, so saving only
    /// keeps the entry recognizable — the `builtin` marker plus the default name / match /
    /// target when the user has not customized them. Deleting a built-in rule is allowed and is
    /// **not** undone here (restoring is an explicit user action); the `builtin` marker of
    /// existing rule set entries is re-normalized but never re-added.
    pub fn save(&self, ovr: &LocalOverride) -> PanelResult<()> {
        let mut ovr = ovr.clone();
        normalize_builtin_route_rules(&mut ovr);
        normalize_builtin_rule_sets(&mut ovr);
        let path = self.override_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&ovr)?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}

/// 存量迁移（幂等）：把旧版内置机制数据归一化为纯用户自控模型，并清空已移除的
/// 场景模板字段。
///
/// 1. `rule_set_subscriptions` 中 `subscribed == true` 的条目 → 转换为启用的
///    [`CustomRuleSet`] Remote（Binary），随后清空整个订阅段（写回空数组
///    无害）。映射：
///    - `id`：新生成 UUID；
///    - `name` = `display_name`；
///    - `tag` = `community_id`；
///    - `source` = `Remote { url: singbox_url_template 的 "{tag}" 替换为
///      community_id, format: Binary }`；
///    - `last_updated: 0`（内置机制无此记录；enabled 概念已随纯资源语义移除）。
/// 2. `applied_templates` / `custom_templates`：场景模板功能已移除，字段仅作
///    serde 兼容读取旧文件，这里一次性清空，写回空数组。
///
/// 返回是否发生了改动；未改动时 `load` 不会触发写盘。
fn migrate_legacy_builtins(ovr: &mut LocalOverride) -> bool {
    let mut changed = false;

    if !ovr.rule_set_subscriptions.is_empty() {
        for sub in ovr.rule_set_subscriptions.iter().filter(|s| s.subscribed) {
            let url = sub.singbox_url_template.replace("{tag}", &sub.community_id);
            ovr.custom_rule_sets.push(CustomRuleSet {
                id: uuid::Uuid::new_v4().to_string(),
                name: sub.display_name.clone(),
                tag: sub.community_id.clone(),
                source: CustomRuleSetSource::Remote {
                    url,
                    format: RuleSetFormat::Binary,
                },
                last_updated: 0,
                remote_updated_at: 0,
                // Migration output is a user entry (not materialized from the current built-in
                // spec), so it is out of scope for "reset built-in rule sets".
                builtin: false,
            });
        }
        ovr.rule_set_subscriptions.clear();
        changed = true;
    }

    // 场景模板字段一次性清空（写回空数组，serde 兼容旧格式）。
    if !ovr.applied_templates.is_empty() {
        ovr.applied_templates.clear();
        changed = true;
    }
    if !ovr.custom_templates.is_empty() {
        ovr.custom_templates.clear();
        changed = true;
    }

    changed
}

/// 存量迁移（幂等）：规则总开关 `singbox.enabled` 已移除。旧文件中
/// `singbox.enabled == false` 时，把意图折叠为「所有规则卡片与规则集引用
/// `enabled = false`」——即原本被总开关整体关闭的内容保持关闭，而
/// `enabled == true`（或字段缺失）时不做任何改动。
///
/// `raw` 是反序列化前的 JSON（`enabled` 字段已被类型 schema 忽略）。写回后
/// [`CoreLocalOverride`] 不再序列化该字段，因此二次 load 不会重复迁移。
///
/// 返回是否发生了改动；未改动时 `load` 不会因本迁移触发写盘。
fn migrate_disabled_master_switch(raw: &Value, ovr: &mut LocalOverride) -> bool {
    let disabled = raw
        .get("singbox")
        .and_then(|singbox| singbox.get("enabled"))
        .and_then(Value::as_bool)
        == Some(false);
    if !disabled {
        return false;
    }

    for rule in &mut ovr.singbox.rules {
        rule.enabled = false;
    }
    for rule_set in &mut ovr.singbox.rule_sets {
        rule_set.enabled = false;
    }
    true
}

#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;

/// Drop legacy references left behind by retired built-ins (idempotent, returns whether
/// changed).
///
/// - `singbox.rule_sets` entries carrying a retired country tag **or** a tag that is now a
///   materialized custom rule set are removed: built-in rule sets are custom entries now, and
///   keeping the legacy reference would inject a duplicate `route.rule_set` entry.
/// - Built-in rules whose id is no longer part of [`crate::core_config::BUILTIN_ROUTE_RULES`]
///   are removed (they only referenced retired rule sets).
///
/// User-created rules / rule sets are never touched: if the user still wants a country split,
/// they re-add the rule set from the market and their own rule keeps working.
fn retire_legacy_builtin_references(ovr: &mut LocalOverride) -> bool {
    let is_retired_tag = |tag: &str| {
        crate::core_config::RETIRED_BUILTIN_RULE_SET_TAGS.contains(&tag)
            || crate::core_config::BUILTIN_RULE_SETS
                .iter()
                .any(|spec| spec.tag == tag)
    };
    let rule_sets_before = ovr.singbox.rule_sets.len();
    ovr.singbox
        .rule_sets
        .retain(|rs| !is_retired_tag(rs.tag.as_str()));

    let rules_before = ovr.singbox.rules.len();
    ovr.singbox.rules.retain(|rule| {
        !rule.builtin
            || crate::core_config::BUILTIN_ROUTE_RULES
                .iter()
                .any(|spec| spec.id == rule.id)
    });

    ovr.singbox.rule_sets.len() != rule_sets_before || ovr.singbox.rules.len() != rules_before
}

/// Built-in rule seeding and normalization (idempotent, returns whether changed).
///
/// - A spec rule that is missing is inserted **at the top** (`sort_order` below every existing
///   rule, so the built-in split is evaluated first), `enabled = true` with the spec's default
///   action.
/// - An existing entry only gets its `builtin` marker enforced; name / match / target / action /
///   `sort_order` / `note` / `created_at` follow user edits, because a built-in rule is an
///   ordinary editable rule (deleting it is allowed — restore is an explicit user action).
fn seed_builtin_route_rules(ovr: &mut LocalOverride) -> bool {
    let mut changed = false;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut insert_at = 0usize;
    for spec in crate::core_config::BUILTIN_ROUTE_RULES {
        match ovr.singbox.rules.iter_mut().find(|r| r.id == spec.id) {
            Some(rule) => {
                if !rule.builtin {
                    rule.builtin = true;
                    changed = true;
                }
            }
            None => {
                // Pin to the top: one below the current minimum (0 when there is no rule yet).
                let top_sort = ovr
                    .singbox
                    .rules
                    .iter()
                    .map(|r| r.sort_order)
                    .min()
                    .unwrap_or(1)
                    - 1;
                // Insert at the head so the stored order matches the rendered order.
                ovr.singbox.rules.insert(
                    insert_at,
                    LocalRule {
                        id: spec.id.to_string(),
                        name: spec.name.to_string(),
                        enabled: true,
                        match_type: RuleMatchType::RuleSet,
                        target: spec.target.to_string(),
                        action: if spec.direct {
                            RuleAction::Direct
                        } else {
                            RuleAction::Proxy
                        },
                        advanced: Default::default(),
                        note: "内置规则：可修改、调整顺序或删除，支持一键还原".to_string(),
                        created_at: now,
                        sort_order: top_sort,
                        builtin: true,
                    },
                );
                insert_at += 1;
                changed = true;
            }
        }
    }
    changed
}

/// Re-assert the `builtin` marker on materialized built-in rules (idempotent, returns whether
/// changed).
///
/// Used by `save`: it never re-creates a deleted built-in rule, so deletions stick and
/// restoring stays an explicit user action.
fn normalize_builtin_route_rules(ovr: &mut LocalOverride) -> bool {
    let mut changed = false;
    for spec in crate::core_config::BUILTIN_ROUTE_RULES {
        if let Some(rule) = ovr.singbox.rules.iter_mut().find(|r| r.id == spec.id)
            && !rule.builtin
        {
            rule.builtin = true;
            changed = true;
        }
    }
    changed
}

/// Built-in rule set seeding (idempotent, returns whether changed).
///
/// Materializes [`crate::core_config::BUILTIN_RULE_SETS`] into `custom_rule_sets` as ordinary
/// Remote/Binary entries so every rule-set picker (route rules, DNS rules, …) offers them and
/// the user can edit / delete them like any other entry. Entries are matched by stable `id`,
/// and a same-`tag` entry already present (e.g. migrated from a legacy subscription) is left
/// alone to avoid a duplicate tag.
fn seed_builtin_rule_sets(ovr: &mut LocalOverride) -> bool {
    let mut changed = false;
    for spec in crate::core_config::BUILTIN_RULE_SETS {
        if let Some(existing) = ovr.custom_rule_sets.iter_mut().find(|rs| rs.id == spec.id) {
            if !existing.builtin {
                existing.builtin = true;
                changed = true;
            }
            continue;
        }
        if ovr.custom_rule_sets.iter().any(|rs| rs.tag == spec.tag) {
            continue;
        }
        ovr.custom_rule_sets.push(CustomRuleSet {
            id: spec.id.to_string(),
            name: spec.name.to_string(),
            tag: spec.tag.to_string(),
            source: CustomRuleSetSource::Remote {
                url: spec.url.to_string(),
                format: RuleSetFormat::Binary,
            },
            last_updated: 0,
            remote_updated_at: 0,
            builtin: true,
        });
        changed = true;
    }
    changed
}

/// Re-assert the `builtin` marker on materialized built-in rule sets (idempotent, returns
/// whether changed).
///
/// Used by `save` and by `load` once seeding has happened: it never re-creates a deleted
/// entry, so user deletions stick.
fn normalize_builtin_rule_sets(ovr: &mut LocalOverride) -> bool {
    let mut changed = false;
    for spec in crate::core_config::BUILTIN_RULE_SETS {
        if let Some(existing) = ovr.custom_rule_sets.iter_mut().find(|rs| rs.id == spec.id)
            && !existing.builtin
        {
            existing.builtin = true;
            changed = true;
        }
    }
    changed
}
