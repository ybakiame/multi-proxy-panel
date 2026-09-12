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
            return Ok(ConfigSlices::default());
        }
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "config_slices.json unreadable, fall back to default"
                );
                return Ok(ConfigSlices::default());
            }
        };
        let value: Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "config_slices.json corrupted, fall back to default"
                );
                return Ok(ConfigSlices::default());
            }
        };
        let mut slices: ConfigSlices = match serde_json::from_value(value.clone()) {
            Ok(slices) => slices,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "config_slices.json schema mismatch, fall back to default"
                );
                return Ok(ConfigSlices::default());
            }
        };
        if migrate_v1_to_v2(&value, &mut slices) {
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
