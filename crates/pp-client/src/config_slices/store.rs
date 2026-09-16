//! Config slice storage: reads/writes `data_dir/config_slices.json`.
//!
//! Follows the same resilience pattern as [`crate::local_override::LocalOverrideStore`]:
//! - Missing file → default.
//! - Corrupted file → log warning, fall back to default (non-blocking).
//! - `#[serde(default)]` on all fields for forward compatibility.
//!
//! `save` validates before writing: an invalid slice set returns a clear
//! [`pp_common::PanelError::Validation`] and never touches the file.
//!
//! `load` runs an idempotent v1 → v2 migration (see [`migrate_v1_to_v2`]) that
//! folds the removed per-slice `enabled` master switches into the remaining
//! fields, then persists the normalized document back to disk.

use std::path::PathBuf;

use pp_common::PanelResult;
use serde_json::Value;

use super::{ConfigSlices, DnsMode, DomainResolverSlice, SLICE_VERSION};

/// Storage for [`ConfigSlices`] at `data_dir/config_slices.json`.
#[derive(Debug, Clone)]
pub struct ConfigSlicesStore {
    data_dir: PathBuf,
}

impl ConfigSlicesStore {
    /// Create storage based on the data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// `data_dir/config_slices.json`.
    #[must_use]
    pub fn slices_file(&self) -> PathBuf {
        self.data_dir.join("config_slices.json")
    }

    /// Load config slices.
    ///
    /// A missing or corrupted file returns [`ConfigSlices::default`] and never
    /// blocks core startup (ADR-0005 §3.6). A v1 document is migrated to v2 and
    /// written back (idempotent: once `version >= 2` no migration runs).
    pub fn load(&self) -> PanelResult<ConfigSlices> {
        let path = self.slices_file();
        if !path.exists() {
            // 文件缺失：播种内置分组条目（2026-09 起内置 proxy/auto 分组物化进出站切片，
            // 可修改不可删除）并落盘，使后续 load 读到归一化数据。
            let mut slices = ConfigSlices::default();
            seed_builtin_outbound_groups(&mut slices);
            if let Err(e) = self.save(&slices) {
                tracing::warn!(error = %e, "failed to persist builtin outbound group seeding");
            }
            return Ok(slices);
        }
        let read_seeded_default = |store: &Self, reason: &str| {
            tracing::warn!(path = %store.slices_file().display(), reason, "fall back to seeded default");
            let mut slices = ConfigSlices::default();
            seed_builtin_outbound_groups(&mut slices);
            slices
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(_) => return Ok(read_seeded_default(self, "config_slices.json unreadable")),
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => return Ok(read_seeded_default(self, "config_slices.json corrupted")),
        };
        let mut slices: ConfigSlices = match serde_json::from_value(value.clone()) {
            Ok(slices) => slices,
            Err(_) => {
                return Ok(read_seeded_default(
                    self,
                    "config_slices.json schema mismatch",
                ));
            }
        };
        let migrated = migrate_v1_to_v2(&value, &mut slices);
        let seeded = seed_builtin_outbound_groups(&mut slices);
        if migrated || seeded {
            // 迁移结果写回磁盘，保证后续 load 读到已归一化数据（幂等）。
            if let Err(e) = self.save(&slices) {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "failed to persist config-slice migration result"
                );
            }
        }
        Ok(slices)
    }

    /// Validate then save config slices to `data_dir/config_slices.json`.
    ///
    /// Validation failures are returned as [`pp_common::PanelError::Validation`]
    /// and the file is left untouched (ADR-0005 §3.5).
    pub fn save(&self, slices: &ConfigSlices) -> PanelResult<()> {
        // 内置分组保护：保存前复活缺失条目并归一化 name/协议/builtin 标记。
        let mut slices = slices.clone();
        seed_builtin_outbound_groups(&mut slices);
        let slices = &slices;
        slices.validate()?;
        let path = self.slices_file();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(slices)?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}

/// 内置分组播种与归一化（幂等，返回是否改动）。
///
/// - 规格（[`crate::core_config::BUILTIN_OUTBOUND_GROUPS`]）缺失的条目：按模板默认值追加
///   （proxy selector default=auto；auto urltest 5m/150ms），`enabled = true`；
/// - 已存在条目：`builtin` 置位、`name` 与协议类型归一化到规格；其余可调字段
///   （default / url / interval / tolerance / interrupt_exist_connections）尊重用户改动。
fn seed_builtin_outbound_groups(slices: &mut ConfigSlices) -> bool {
    let mut changed = false;
    for spec in &crate::core_config::BUILTIN_OUTBOUND_GROUPS {
        match slices.outbounds.items.iter_mut().find(|i| i.id == spec.id) {
            Some(item) => {
                let expected_selector = spec.selector;
                let kind_matches = matches!(
                    (&item.protocol, expected_selector),
                    (crate::config_slices::OutboundProtocol::Selector(_), true)
                        | (crate::config_slices::OutboundProtocol::UrlTest(_), false)
                );
                if !item.builtin || item.name != spec.name || !kind_matches {
                    item.builtin = true;
                    item.name = spec.name.to_string();
                    if !kind_matches {
                        item.protocol = default_builtin_group_protocol(spec);
                    }
                    changed = true;
                }
            }
            None => {
                slices
                    .outbounds
                    .items
                    .push(crate::config_slices::CustomOutbound {
                        id: spec.id.to_string(),
                        name: spec.name.to_string(),
                        enabled: true,
                        builtin: true,
                        protocol: default_builtin_group_protocol(spec),
                    });
                changed = true;
            }
        }
    }
    changed
}

/// 内置分组条目的模板默认协议体（对齐 `singbox_template` 生成的 proxy/auto 分组）。
fn default_builtin_group_protocol(
    spec: &crate::core_config::BuiltinOutboundGroupSpec,
) -> crate::config_slices::OutboundProtocol {
    if spec.selector {
        crate::config_slices::OutboundProtocol::Selector(crate::config_slices::SelectorOutbound {
            outbounds: vec![crate::core_config::OUTBOUND_TAG_AUTO.to_string()],
            default: crate::core_config::OUTBOUND_TAG_AUTO.to_string(),
            interrupt_exist_connections: false,
        })
    } else {
        crate::config_slices::OutboundProtocol::UrlTest(crate::config_slices::UrlTestOutbound {
            outbounds: Vec::new(),
            url: "https://www.gstatic.com/generate_204".to_string(),
            interval: "5m".to_string(),
            tolerance: 150,
            interrupt_exist_connections: false,
        })
    }
}

/// Idempotent v1 → v2 migration: the per-slice `enabled` master switches were
/// removed, so a disabled slice must fold its intent into the remaining fields.
///
/// | v1 field                  | v1 `enabled == false` → v2 |
/// |---------------------------|----------------------------|
/// | `outbounds.enabled`       | every `items[*].enabled = false` |
/// | `route.enabled`           | clear `final_tag` + `resolver` |
/// | `experimental.enabled`    | `cache_file.enabled = false` |
/// | `dns.enabled`             | `mode = follow_system` |
///
/// `raw` is the pre-deserialization JSON (the removed fields are ignored by the
/// typed schema). Always bumps `version` to [`SLICE_VERSION`] and returns
/// `true` whenever the stored version is older, so the normalized document is
/// persisted once. A version already at (or above) v2 is a no-op.
fn migrate_v1_to_v2(raw: &Value, slices: &mut ConfigSlices) -> bool {
    if slices.version >= SLICE_VERSION {
        return false;
    }

    if raw_field(raw, "outbounds", "enabled") == Some(false) {
        for item in &mut slices.outbounds.items {
            item.enabled = false;
        }
    }
    if raw_field(raw, "route", "enabled") == Some(false) {
        slices.route.final_tag.clear();
        slices.route.resolver = DomainResolverSlice::default();
    }
    if raw_field(raw, "experimental", "enabled") == Some(false) {
        slices.experimental.cache_file.enabled = false;
    }
    if raw_field(raw, "dns", "enabled") == Some(false) {
        slices.dns.mode = DnsMode::FollowSystem;
    }

    slices.version = SLICE_VERSION;
    true
}

/// Read `raw[section][field]` as a bool, if present.
fn raw_field(raw: &Value, section: &str, field: &str) -> Option<bool> {
    raw.get(section)?.get(field)?.as_bool()
}

#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;
