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
                builtin: false,
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
    // 2026-09 起：文件缺失也播种内置分组（proxy/auto，可修改不可删除）并落盘。
    assert_eq!(slices.outbounds.items.len(), 4);
    assert!(slices.outbounds.items.iter().all(|i| i.builtin));
    assert_eq!(slices.outbounds.items[0].id, "builtin-group-proxy");
    assert_eq!(slices.outbounds.items[1].id, "builtin-group-auto");
    assert_eq!(slices.outbounds.items[2].id, "builtin-group-global");
    assert_eq!(slices.outbounds.items[3].id, "builtin-group-final");
    assert!(store.slices_file().exists(), "seeding persists the file");
}

#[test]
fn load_corrupted_file_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    std::fs::write(store.slices_file(), "not valid json {{{").unwrap();
    let slices = store.load().unwrap();
    // 损坏回退到播种默认值（含 2 条内置分组）。
    assert_eq!(slices.outbounds.items.len(), 4);
    assert!(slices.outbounds.items.iter().all(|i| i.builtin));
}

#[test]
fn save_then_load_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());
    let slices = valid_slices();
    store.save(&slices).unwrap();
    let loaded = store.load().unwrap();
    // save 播种内置分组：loaded = 用户条目 + 2 条内置条目（追加在后）。
    assert_eq!(loaded.outbounds.items.len(), 5);
    assert_eq!(loaded.outbounds.items[0].id, "o1");
    assert!(loaded.outbounds.items[1..].iter().all(|i| i.builtin));
    // 用户条目逐字段不变。
    assert_eq!(loaded.outbounds.items[0], slices.outbounds.items[0]);
    assert_eq!(loaded.dns, slices.dns);
    assert_eq!(loaded.experimental, slices.experimental);
    assert_eq!(loaded.route, slices.route);
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
    // save 会播种/归一化内置分组：loaded 比原始输入多 2 条内置条目。
    let mut expected = slices.clone();
    expected
        .outbounds
        .items
        .extend(loaded.outbounds.items.iter().filter(|i| i.builtin).cloned());
    assert_eq!(loaded, expected);
    let after = std::fs::read_to_string(store.slices_file()).unwrap();
    assert_eq!(before, after, "v2 load must not rewrite the file");
}

/// 内置分组「可修改不可删除」：save 时缺失条目复活、name/协议归一化；可调字段保留。
#[test]
fn save_resurrects_and_normalizes_builtin_groups() {
    let dir = tempfile::tempdir().unwrap();
    let store = ConfigSlicesStore::new(dir.path().to_path_buf());

    // 空保存 → 复活两条内置分组。
    store.save(&ConfigSlices::default()).unwrap();
    let loaded = store.load().unwrap();
    assert_eq!(loaded.outbounds.items.len(), 4);
    assert!(loaded.outbounds.items.iter().all(|i| i.builtin));

    // 篡改 name / 协议类型，调整可调字段：保存后前者归一、后者保留。
    let mut slices = loaded;
    let proxy = slices
        .outbounds
        .items
        .iter_mut()
        .find(|i| i.id == "builtin-group-proxy")
        .unwrap();
    proxy.name = "tampered".to_string();
    proxy.protocol = OutboundProtocol::Selector(SelectorOutbound {
        outbounds: vec![],
        default: "slice-x".to_string(),
        interrupt_exist_connections: true,
    });
    store.save(&slices).unwrap();

    let loaded = store.load().unwrap();
    let proxy = loaded
        .outbounds
        .items
        .iter()
        .find(|i| i.id == "builtin-group-proxy")
        .unwrap();
    assert_eq!(proxy.name, "proxy");
    match &proxy.protocol {
        OutboundProtocol::Selector(sel) => {
            assert_eq!(sel.default, "slice-x", "user-tunable default preserved");
            assert!(sel.interrupt_exist_connections);
        }
        _ => panic!("protocol must stay selector"),
    }
}
