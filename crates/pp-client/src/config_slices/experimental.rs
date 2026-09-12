//! Experimental slice schema (ADR-0005 P1).
//!
//! Curated projection of the sing-box top-level `experimental` section, limited
//! to `cache_file`. `clash_api` stays owned by the panel-feature layer (④) and
//! is intentionally not part of this slice; `v2ray_api` / `debug` are out of
//! scope.
//!
//! Rendering lives in [`crate::config_slices::apply_config_slices`], which deep
//! merges `cache_file` into any existing `experimental` object so sibling keys
//! (`clash_api`, …) survive.

use serde::{Deserialize, Serialize};

/// Experimental slice: structured form of the sing-box `experimental` section.
///
/// All fields are `#[serde(default)]`; a missing or partial object deserializes
/// to sensible defaults. No master switch: injection is driven by
/// [`CacheFileSlice::enabled`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentalSlice {
    /// Persistent cache file (sing-box `experimental.cache_file`).
    #[serde(default)]
    pub cache_file: CacheFileSlice,
}

/// sing-box `experimental.cache_file` (curated fields).
///
/// Field set verified against sing-box 1.14 `option.CacheFileOptions`:
/// `enabled` / `path` / `cache_id` / `store_fakeip`. The 1.14-deprecated
/// `store_rdrc` / `rdrc_timeout` are never emitted, and the 1.14-added
/// `store_dns` is left out of the curated scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheFileSlice {
    /// Whether the cache file is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Path to the cache file (empty = sing-box default `cache.db`).
    #[serde(default)]
    pub path: String,
    /// Identifier for a separate store inside the cache file (empty = none).
    #[serde(default)]
    pub cache_id: String,
    /// Store fakeip records in the cache file.
    #[serde(default)]
    pub store_fakeip: bool,
}

#[cfg(test)]
#[path = "tests/experimental_tests.rs"]
mod tests;
