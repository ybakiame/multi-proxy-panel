//! Config slice storage: reads/writes `data_dir/config_slices.json`.
//!
//! Follows the same resilience pattern as [`crate::local_override::LocalOverrideStore`]:
//! - Missing file → default (all slices disabled).
//! - Corrupted file → log warning, fall back to default (non-blocking).
//! - `#[serde(default)]` on all fields for forward compatibility.
//!
//! `save` validates before writing: an invalid slice set returns a clear
//! [`pp_common::PanelError::Validation`] and never touches the file.

use std::path::PathBuf;

use pp_common::PanelResult;

use super::ConfigSlices;

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
    /// blocks core startup (ADR-0005 §3.6).
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
        match serde_json::from_str(&text) {
            Ok(slices) => Ok(slices),
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "config_slices.json corrupted, fall back to default"
                );
                Ok(ConfigSlices::default())
            }
        }
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

#[cfg(test)]
#[path = "tests/store_tests.rs"]
mod tests;
