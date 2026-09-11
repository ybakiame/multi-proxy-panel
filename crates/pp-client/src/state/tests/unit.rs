//! Unit tests for state helpers (compat checks, redaction, platform overrides).

use super::helpers::*;
use crate::state::compat;

/// Compile-time assertion: `ClientState::start` future is `Send` (`apply_js_override`
/// no longer contains rquickjs non-`Send` structures after being driven by [`ScriptWorker`], can be awaited across threads).
#[test]
fn client_state_start_future_is_send() {
    fn assert_send<T: Send>(_: &T) {}
    let dir = tempfile::tempdir().unwrap();
    let mut state = ClientState::new(test_config(&dir, String::new()));
    let fut = state.start();
    assert_send(&fut);
}

/// Item 1 regression: start always reloads config from disk.
///
/// Disk client.json `tun_enabled=false`, but instance caches old snapshot `tun_enabled=true`
/// (simulates `ClientState` created before user turned off TUN); after start, composed config received by core
/// must not contain tun inbound (user reported "TUN turned off in settings but still injects tun-in on startup").
#[tokio::test]
async fn start_reloads_disk_config_so_tun_toggle_takes_effect() {
    let body = r#"{
            "log": {"level": "info"},
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } }
            ],
            "route": { "final": "direct" }
        }"#;
    let addr = spawn_server(StatusCode::OK, body).await;
    let dir = tempfile::tempdir().unwrap();
    let sub_id = add_local_subscription(&dir, &format!("http://{addr}/sub"));

    // Disk config: TUN is turned off (user-reported scenario).
    let capture = dir.path().join("core-config-capture.json");
    let core_bin = fake_core_capturing_args(&dir, &capture);
    let mut disk = ClientConfig::new(
        dir.path().to_path_buf(),
        format!("http://{addr}"),
        "tok",
        core_bin,
    );
    disk.active_subscription_id = Some(sub_id);
    disk.tun_enabled = false;
    // 本测试只验证启动时从磁盘重载 TUN 开关，断言不依赖 Clash API；
    // 无真实 Clash API 服务，关闭以免启动阻塞在 readiness 等待 + 模式推送重试。
    disk.clash_api_enabled = false;
    disk.save().unwrap();

    // Cached old snapshot: TUN still on (simulates ClientState created before user turned off TUN).
    let mut stale = disk.clone();
    stale.tun_enabled = true;

    let mock = Arc::new(MockSystemProxy::new());
    // Disk tun_enabled=false: after reload takes effect, TUN pre-start privilege check is not triggered,
    // use default permission detection (if reload fails, start would fail with NeedsAuth error).
    let mut state = ClientState::with_system_proxy(stale, mock.clone());
    state.start().await.unwrap();

    let mut attempts = 0;
    while !capture.exists() && attempts < 100 {
        tokio::time::sleep(Duration::from_millis(50)).await;
        attempts += 1;
    }
    assert!(
        capture.exists(),
        "fake core should have copied composed config"
    );
    let core_config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&capture).unwrap()).unwrap();
    let inbounds = core_config["inbounds"].as_array().unwrap();
    assert!(
        !inbounds.iter().any(|i| i["type"] == "tun"),
        "should not inject tun inbound when disk tun_enabled=false: {core_config}"
    );

    state.stop().await;
}

/// Item 2: `tun_enabled=true` but core is not authorized → start returns `tun_auth_required` error,
/// core does not start, system proxy zero calls (frontend shows authorization entry accordingly).
#[tokio::test]
async fn start_requires_tun_authorization_when_tun_enabled() {
    let body = r#"{
            "log": {"level": "info"},
            "outbounds": [{"type": "direct"}]
        }"#;
    let addr = spawn_server(StatusCode::OK, body).await;
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = test_config(&dir, format!("http://{addr}"));
    cfg.tun_enabled = true;
    cfg.save().unwrap();
    let binary = cfg.core_binary.clone();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    let err = state.start().await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("tun_auth_required"),
        "should contain tun_auth_required marker: {msg}"
    );
    assert!(
        msg.contains(binary.to_string_lossy().as_ref()),
        "should contain core binary path: {msg}"
    );

    // Core not started, system proxy zero calls.
    let status = state.status().await;
    assert!(!status.core_running);
    assert_eq!(mock.calls(), vec![]);
}

/// Android semantics: `apply_android_overrides` disables desktop-exclusive features
/// (MITM / system proxy / TUN desktop semantics) and forces the Clash API mandatory slice on.
/// Desktop builds cannot execute Android path, so extract pure function and test its semantics on desktop.
#[test]
fn apply_android_overrides_forces_android_semantics() {
    let mut cfg = ClientConfig {
        mitm_enabled: true,
        system_proxy_enabled: true,
        tun_enabled: true,
        clash_api_enabled: false,
        ..ClientConfig::default()
    };
    compat::apply_android_overrides(&mut cfg);
    assert!(!cfg.mitm_enabled, "Android disables MITM");
    assert!(!cfg.system_proxy_enabled, "Android disables system proxy");
    assert!(!cfg.tun_enabled, "Android disables TUN (desktop semantics)");
    assert!(
        cfg.clash_api_enabled,
        "Android forces Clash API enabled (mandatory slice)"
    );
}

/// Android config composition TUN toggle: sing-box needs tun inbound to callback
/// openTun to establish VPN interface (always true); desktop passes through user settings as-is.
#[test]
fn panel_features_tun_enabled_android_and_desktop_semantics() {
    // Android → always true (needs tun inbound to trigger openTun).
    assert!(
        compat::panel_features_tun_enabled(true, false),
        "Android always enables TUN"
    );
    assert!(
        compat::panel_features_tun_enabled(true, true),
        "Android always enables TUN"
    );
    // Desktop passes through user settings as-is.
    assert!(
        !compat::panel_features_tun_enabled(false, false),
        "desktop keeps TUN off when off"
    );
    assert!(
        compat::panel_features_tun_enabled(false, true),
        "desktop keeps TUN on when on"
    );
}

/// Redaction: uuid/password/server in nested outbounds are replaced with "***",
/// non-matching keys (type / tag / server_name) are preserved as-is.
#[test]
fn redact_config_credentials_masks_outbound_credentials() {
    let mut cfg = serde_json::json!({
        "outbounds": [
            {
                "type": "vless",
                "tag": "n1",
                "server": "example.com",
                "uuid": "12345678-1234-1234-1234-123456789012",
                "tls": { "enabled": true, "server_name": "example.com" }
            },
            {
                "type": "hysteria2",
                "tag": "n2",
                "server": "example.org",
                "password": "pw"
            }
        ]
    });
    compat::redact_config_credentials(&mut cfg);

    let n1 = &cfg["outbounds"][0];
    assert_eq!(n1["server"], "***");
    assert_eq!(n1["uuid"], "***");
    assert_eq!(n1["type"], "vless", "non-matching key unaffected");
    assert_eq!(n1["tag"], "n1", "non-matching key unaffected");
    assert_eq!(
        n1["tls"]["server_name"], "example.com",
        "server_name non-matching key unaffected"
    );
    let n2 = &cfg["outbounds"][1];
    assert_eq!(n2["server"], "***");
    assert_eq!(n2["password"], "***");
}

/// Redaction: password in users array is replaced with "***".
#[test]
fn redact_config_credentials_masks_users_array_passwords() {
    let mut cfg = serde_json::json!({
        "inbounds": [
            {
                "type": "shadowsocks",
                "tag": "in",
                "users": [
                    { "username": "alice", "password": "secret1" },
                    { "username": "bob", "password": "secret2" }
                ]
            }
        ]
    });
    compat::redact_config_credentials(&mut cfg);

    let users = &cfg["inbounds"][0]["users"];
    assert_eq!(users[0]["password"], "***");
    assert_eq!(users[1]["password"], "***");
    assert_eq!(
        users[0]["username"], "alice",
        "username non-matching key unaffected"
    );
}

/// Redaction: dns servers structure is preserved (detour key kept as-is), server values are masked.
#[test]
fn redact_config_credentials_preserves_dns_structure_but_masks_server() {
    let mut cfg = serde_json::json!({
        "dns": {
            "servers": [
                { "tag": "dns-main", "server": "8.8.8.8" },
                { "tag": "dns-proxy", "server": "1.1.1.1", "detour": "proxy" }
            ]
        }
    });
    compat::redact_config_credentials(&mut cfg);

    let servers = &cfg["dns"]["servers"];
    assert_eq!(
        servers.as_array().unwrap().len(),
        2,
        "structure preserved as-is"
    );
    assert_eq!(servers[0]["server"], "***");
    assert_eq!(servers[1]["server"], "***");
    assert_eq!(servers[1]["detour"], "proxy", "detour key preserved as-is");
    assert_eq!(servers[0]["tag"], "dns-main");
}

/// Redaction: non-matching keys and non-string values are unaffected; non-string values still recurse their child structures.
#[test]
fn redact_config_credentials_keeps_unmatched_and_non_string_values() {
    let mut cfg = serde_json::json!({
        "log": { "level": "info" },
        "route": { "final": "direct" },
        "experimental": {
            "server_port": 443,
            "server_name": "keep.example.com",
            "server": { "host": "deep.example.com" },
            "uuid": 123
        }
    });
    compat::redact_config_credentials(&mut cfg);

    assert_eq!(cfg["log"]["level"], "info");
    assert_eq!(cfg["route"]["final"], "direct");
    assert_eq!(
        cfg["experimental"]["server_port"], 443,
        "non-string server_port not replaced"
    );
    assert_eq!(
        cfg["experimental"]["server_name"], "keep.example.com",
        "server_name non-matching key unaffected"
    );
    assert_eq!(
        cfg["experimental"]["uuid"], 123,
        "non-string uuid not replaced"
    );
    // server key value is non-string: not replaced but still recurses its child structure.
    assert!(
        cfg["experimental"]["server"].is_object(),
        "non-string server preserved as object"
    );
    assert_eq!(cfg["experimental"]["server"]["host"], "deep.example.com");
}
