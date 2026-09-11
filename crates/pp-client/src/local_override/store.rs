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

use std::path::PathBuf;

use pp_common::PanelResult;
use serde_json::Value;

use super::{CustomRuleSet, CustomRuleSetSource, LocalOverride, RuleSetFormat};

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
    /// - Legacy file → runs the idempotent built-in mechanism migration and clears
    ///   the removed scenario-template fields, then best-effort persists the
    ///   normalized state back to disk.
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
        let value: Value = match serde_json::from_str(&text) {
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
        let migrated = migrate_legacy_builtins(&mut ovr);
        if migrated {
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

#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;
