//! Local Override storage: reads/writes `data_dir/local_override.json`.
//!
//! Follows the same resilience pattern as [`ProfileStoreV2`]:
//! - Missing file → default (empty) config.
//! - Corrupted file → log warning, fall back to default (non-blocking).
//! - `#[serde(default)]` on all fields for forward compatibility.
//!
//! 自「废弃内置规则集订阅与内置模板」起，`load` 内嵌**幂等存量迁移**：把旧版
//! 已订阅内置条目转换为用户自控的 custom Remote 规则集、清理内置模板记录，
//! 使旧文件一次读取即归一化到纯用户自控模型。
//!
//! 自「场景模板改为规则引用 + 应用激活」起，`load` 额外在反序列化**之前**做一次
//! 快照→引用迁移：旧 `custom_templates[].rules`（规则对象数组）→ 规则 ID 引用
//! 列表（数组首遍为对象、含原规则 `id`）；转换后写回，二次读取幂等。旧
//! `custom_rule_sets[].enabled` 字段由 serde 静默忽略（纯资源语义，无启用概念）。

use std::path::PathBuf;

use pp_common::PanelResult;
use serde_json::Value;

use super::{CustomRuleSet, CustomRuleSetSource, LocalOverride, RuleSetFormat};
use crate::local_override::template::CUSTOM_TEMPLATE_PREFIX;

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
    /// - File missing → returns default (empty) config with enabled = true.
    /// - File corrupted → logs warning, returns default (does not block startup).
    /// - Legacy file → runs the idempotent built-in mechanism migration (and,
    ///   since the reference-semantics refactor, the snapshot→reference template
    ///   migration) and best-effort persists the normalized state back to disk.
    pub fn load(&self) -> PanelResult<LocalOverride> {
        let path = self.override_file();
        if !path.exists() {
            return Ok(LocalOverride::default());
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
        // Value-level pre-pass: legacy `custom_templates[].rules` snapshots must
        // be converted to ID references *before* typed deserialization (the new
        // `rules: Vec<String>` type cannot parse an array of rule objects).
        let mut value: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local_override.json corrupted, fall back to default"
                );
                return Ok(LocalOverride::default());
            }
        };
        let snapshot_migrated = migrate_legacy_template_snapshots(&mut value);
        let mut ovr: LocalOverride = match serde_json::from_value(value) {
            Ok(o) => o,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "local_override.json schema mismatch, fall back to default"
                );
                return Ok(LocalOverride::default());
            }
        };
        let builtins_migrated = migrate_legacy_builtins(&mut ovr);
        if snapshot_migrated || builtins_migrated {
            // 迁移结果写回磁盘，保证后续 load 读到已归一化数据（幂等）。
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
    pub fn save(&self, ovr: &LocalOverride) -> PanelResult<()> {
        let path = self.override_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(ovr)?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}

/// 存量迁移（幂等，反序列化前置）：把旧版 `custom_templates[].rules` 的**规则
/// 快照数组**转换为**规则 ID 引用列表**。
///
/// 引用语义下模板只关心规则 ID（是否注入取决于「规则存在且启用 + 模板已应用」），
/// 原快照的规则定义不再需要；但规则本身仍是用户列表中的卡片（未被删除的 ID
/// 引用在迁移后照常注入）。迁移映射：`rules: [{ ...full rule, "id": "r1" }]`
/// → `["r1", ...]`。已是字符串引用数组的模板不动（幂等）。
///
/// 返回是否发生了改动；未改动时 `load` 不会触发写盘。
fn migrate_legacy_template_snapshots(value: &mut Value) -> bool {
    let Some(templates) = value
        .get_mut("custom_templates")
        .and_then(|v| v.as_array_mut())
    else {
        return false;
    };
    let mut changed = false;
    for tpl in templates {
        let Some(obj) = tpl.as_object_mut() else {
            continue;
        };
        let Some(rules) = obj.get_mut("rules") else {
            continue;
        };
        let Some(items) = rules.as_array() else {
            continue;
        };
        // 新格式是字符串 ID 数组；旧格式是（至少一个）规则对象数组。
        if !items.iter().any(Value::is_object) {
            continue;
        }
        let refs: Vec<Value> = items
            .iter()
            .filter_map(|item| item.get("id").cloned())
            .collect();
        *rules = Value::Array(refs);
        changed = true;
    }
    changed
}

/// 存量迁移（幂等）：把旧版内置机制数据归一化为纯用户自控模型。
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
/// 2. `applied_templates` 中 `template_id` **不带** `custom:` 前缀的记录删除
///    （内置模板撤销入口随模板移除）；这些记录生成的规则**保留**在
///    `singbox.rules`，由用户手动管理。
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
            });
        }
        ovr.rule_set_subscriptions.clear();
        changed = true;
    }

    let before = ovr.applied_templates.len();
    ovr.applied_templates
        .retain(|t| t.template_id.starts_with(CUSTOM_TEMPLATE_PREFIX));
    changed |= ovr.applied_templates.len() != before;

    changed
}

#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;
