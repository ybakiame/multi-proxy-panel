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
            enabled: true,
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
    slices.dns.enabled = true;
    slices.dns.mode = DnsMode::Takeover;
    slices.dns.final_tag = "dangling".to_string();

    let err = store.save(&slices).unwrap_err();
    assert!(matches!(err, pp_common::PanelError::Validation(_)), "{err}");
    assert!(!store.slices_file().exists());
}
