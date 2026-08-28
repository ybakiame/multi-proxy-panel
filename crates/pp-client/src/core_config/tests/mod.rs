use crate::core_config::*;
use std::path::PathBuf;

/// Real core binary directory: `target/test-cores` (under workspace root). Skip when missing.
fn test_core_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/test-cores")
}

fn sing_box_binary() -> Option<PathBuf> {
    let p = test_core_dir().join("sing-box");
    p.is_file().then_some(p)
}

fn mihomo_binary() -> Option<PathBuf> {
    let p = test_core_dir().join("mihomo");
    p.is_file().then_some(p)
}

/// Locally downloaded mihomo geoip.metadb (`~/.config/mihomo`), avoid `mihomo -t` downloading geo data.
fn geoip_metadb() -> Option<PathBuf> {
    let p = PathBuf::from(std::env::var("HOME").unwrap_or_default())
        .join(".config/mihomo/geoip.metadb");
    p.is_file().then_some(p)
}

#[cfg(test)]
mod clash_api;
#[cfg(test)]
mod compose;
#[cfg(test)]
mod features;
#[cfg(test)]
mod real_core;
#[cfg(test)]
mod ui;
