//! Store tests for `ConfigSlicesStore`.
//!
//! Split out of `store.rs` to stay within the business-file size gate
//! (`.agents/rules/code-organization.md`).

use crate::config_slices::*;

fn valid_slices() -> ConfigSlices {
    ConfigSlices {
        version: SLICE_VERSION,
        dns: DnsSlice::default(),
        outbounds: OutboundsSlice {
            items: vec![CustomOutbound {
                id: "o1".to_string(),
                name: "Node One".to_string(),
                enabled: true,
                protocol: OutboundProtocol::Shadowsocks(ShadowsocksOutbound {
                    server: "1.2.3.4".to_string(),
                    server_port: 8388,
                    method: "aes-256-gcm".to_string(),
                    password: "secret".to_string(),
                }),
            }],
        },
        experimental: ExperimentalSlice::default(),
        route: RouteSlice::default(),
    }
}

#[test]
fn load_missing_file_returns_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    let slices = store.load().unwrap();
    assert_eq!(slices, ConfigSlices::default());
    assert!(!store.slices_file().exists());
}

#[test]
fn load_corrupted_file_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    std::fs::write(store.slices_file(), "not valid json {{{").unwrap();
    let slices = store.load().unwrap();
    assert_eq!(slices, ConfigSlices::default());
}

#[test]
fn save_then_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    let slices = valid_slices();
    store.save(&slices).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(slices, loaded);
}

#[test]
fn save_creates_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("nested/data");
    let store = ConfigSlicesStore::new(nested);
    store.save(&valid_slices()).unwrap();
    assert!(store.slices_file().exists());
}

#[test]
fn save_rejects_invalid_and_does_not_write() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    let mut slices = valid_slices();
    slices.dns.mode = DnsMode::Takeover;
    slices.dns.final_tag = "dangling".to_string();

    let err = store.save(&slices).unwrap_err();
    assert!(matches!(err, pp_common::PanelError::Validation(_)), "{err}");
    assert!(!store.slices_file().exists());
}

// -----------------------------------------------------------------------
// v1 → v2 migration (removed per-slice `enabled` master switches)
// -----------------------------------------------------------------------

fn write_v1(dir: &tempfile::TempDir, json: serde_json::Value) -> ConfigSlicesStore {
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    std::fs::write(
        store.slices_file(),
        serde_json::to_string_pretty(&json).unwrap(),
    )
    .unwrap();
    store
}

/// A v1 document with every master switch off folds into the remaining fields
/// and is persisted as v2.
#[test]
fn migration_folds_disabled_master_switches_into_content() {
    let dir = tempfile::tempdir().unwrap();
    let store = write_v1(
        &dir,
        serde_json::json!({
            "version": 1,
            "dns": {
                "enabled": false,
                "mode": "takeover",
                "servers": [{ "tag": "remote", "type": "https", "server": "1.1.1.1" }],
                "final_tag": "remote"
            },
            "outbounds": {
                "enabled": false,
                "items": [{
                    "id": "o1", "name": "Node", "enabled": true, "type": "shadowsocks",
                    "server": "1.2.3.4", "server_port": 8388,
                    "method": "aes-256-gcm", "password": "x"
                }]
            },
            "experimental": {
                "enabled": false,
                "cache_file": { "enabled": true, "path": "/data/cache.db" }
            },
            "route": {
                "enabled": false,
                "final_tag": "direct",
                "resolver": { "server": "remote" }
            }
        }),
    );

    let slices = store.load().unwrap();
    assert_eq!(slices.version, SLICE_VERSION);
    // outbounds.enabled=false → all items disabled.
    assert!(!slices.outbounds.items[0].enabled);
    // route.enabled=false → final/resolver cleared.
    assert!(slices.route.final_tag.is_empty());
    assert!(slices.route.resolver.server.is_empty());
    // experimental.enabled=false → cache_file disabled.
    assert!(!slices.experimental.cache_file.enabled);
    // dns.enabled=false → follow_system.
    assert_eq!(slices.dns.mode, DnsMode::FollowSystem);

    // Normalized v2 document written back: reload is a no-op migration.
    let reloaded = store.load().unwrap();
    assert_eq!(reloaded, slices);
    let text = std::fs::read_to_string(store.slices_file()).unwrap();
    assert!(text.contains("\"version\": 2"), "{text}");
    let written: serde_json::Value = serde_json::from_str(&text).unwrap();
    // Slice-level master switches are gone (item/cache_file `enabled` remain).
    for section in ["dns", "outbounds", "experimental", "route"] {
        assert!(
            written[section].get("enabled").is_none(),
            "{section} master switch must be gone: {text}"
        );
    }
}

/// v1 document with master switches on keeps its content (only `version` bumps).
#[test]
fn migration_keeps_content_when_master_switches_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let store = write_v1(
        &dir,
        serde_json::json!({
            "version": 1,
            "dns": {
                "enabled": true,
                "mode": "takeover",
                "servers": [{ "tag": "remote", "type": "https", "server": "1.1.1.1" }],
                "final_tag": "remote"
            },
            "outbounds": {
                "enabled": true,
                "items": [{
                    "id": "o1", "name": "Node", "enabled": true, "type": "shadowsocks",
                    "server": "1.2.3.4", "server_port": 8388,
                    "method": "aes-256-gcm", "password": "x"
                }]
            },
            "experimental": {
                "enabled": true,
                "cache_file": { "enabled": true, "path": "/data/cache.db" }
            },
            "route": {
                "enabled": true,
                "final_tag": "direct",
                "resolver": { "server": "remote" }
            }
        }),
    );

    let slices = store.load().unwrap();
    assert_eq!(slices.version, SLICE_VERSION);
    assert!(slices.outbounds.items[0].enabled);
    assert_eq!(slices.route.final_tag, "direct");
    assert_eq!(slices.route.resolver.server, "remote");
    assert!(slices.experimental.cache_file.enabled);
    assert_eq!(slices.dns.mode, DnsMode::Takeover);
}

/// Already-v2 documents are not rewritten by `load`.
#[test]
fn migration_is_noop_for_v2_document() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    let slices = valid_slices();
    store.save(&slices).unwrap();
    let before = std::fs::read_to_string(store.slices_file()).unwrap();

    let loaded = store.load().unwrap();
    assert_eq!(loaded, slices);
    let after = std::fs::read_to_string(store.slices_file()).unwrap();
    assert_eq!(before, after, "v2 load must not rewrite the file");
}
