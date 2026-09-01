//! Integration tests for `ClientState::start` (spawn fake cores, rollback paths).

use super::helpers::*;

/// Fake core script must be able to be truly started/stopped by CoreManager, otherwise rollback tests are not representative.
#[tokio::test]
async fn fake_core_binary_starts_and_stops() {
    let dir = tempfile::tempdir().unwrap();
    let core_bin = fake_core_script(&dir);
    let runner =
        crate::runner::CoreRunner::create(CoreType::SingBox, &core_bin, dir.path()).unwrap();
    let config = serde_json::json!({"log": {"level": "info"}});
    runner.start(&config).await.unwrap();
    assert!(runner.is_running().await);
    runner.stop().await.unwrap();
    assert!(!runner.is_running().await);
}

/// Subscription failure → do not start core / MITM, and do not enable system proxy.
#[tokio::test]
async fn start_rolls_back_on_subscription_failure() {
    let addr = spawn_server(StatusCode::INTERNAL_SERVER_ERROR, "oops").await;
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = test_config(&dir, format!("http://{addr}"));
    cfg.system_proxy_enabled = true;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    assert!(state.start().await.is_err());

    // Subscription failure: system proxy was never enabled.
    assert_eq!(mock.calls(), vec![]);
    let status = state.status().await;
    assert!(!status.core_running);
    assert!(status.mitm_addr.is_none());
}

/// MITM build failure (CA directory occupied by file) → rollback core, and do not enable system proxy.
#[tokio::test]
async fn start_rolls_back_when_mitm_build_fails() {
    let body = r#"{
            "log": {"level": "info"},
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 1}],
            "outbounds": [{"type": "direct"}]
        }"#;
    let addr = spawn_server(StatusCode::OK, body).await;
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = test_config(&dir, format!("http://{addr}"));
    cfg.system_proxy_enabled = true;
    cfg.mitm_enabled = true;
    cfg.save().unwrap();
    // ca_dir occupied by regular file → FileCaStore cannot write CA → build_mitm_proxy fails.
    std::fs::write(dir.path().join("certs"), b"i am a file").unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    assert!(state.start().await.is_err());

    // MITM build failure: system proxy not enabled, and core has been rolled back and stopped.
    assert_eq!(mock.enable_count(), 0);
    let status = state.status().await;
    assert!(status.mitm_addr.is_none());
    assert!(!status.core_running);
}

/// Full integration server (no external network):
/// `/sub/{token}` subscription config, `/snippet` remote QX snippet (references same server's script URL).
async fn spawn_integration_server() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    // Subscription content now only takes nodes: contains 2 leaf nodes + selector / direct (filtered during extraction).
    let sub_body = r#"{
            "log": {"level": "debug"},
            "inbounds": [{"type": "mixed", "listen": "127.0.0.1", "listen_port": 1}],
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } },
                { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
                  "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
        }"#;
    let snippet = format!(
        "[rewrite_local]\n\
             ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
             ^https?://example\\.com/rsp script-response-body {base}/hook.js\n\
             \n\
             [task_local]\n\
             0 9 * * * {base}/task.js, tag=每日签到\n\
             \n\
             [mitm]\n\
             hostname = *.example.com, api.example2.com\n"
    );
    let app = axum::Router::new()
        .route(
            "/sub/{token}",
            axum::routing::get(move || async move { sub_body }),
        )
        .route(
            "/snippet",
            axum::routing::get(move || async move { snippet }),
        )
        .route(
            "/hook.js",
            axum::routing::get(|| async { "const hook = 1;" }),
        )
        .route(
            "/task.js",
            axum::routing::get(|| async {
                "const task = 2; $notify(\"签到成功\", \"test\", \"hello\"); $done({code: 0});"
            }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    base
}

/// Remote snippet cache → MITM (rewrite/hooks/hostnames injection) + cron scheduler (task scripts).
#[tokio::test]
async fn start_with_remote_snippet_runs_mitm_and_scheduler() {
    let base = spawn_integration_server().await;
    let dir = tempfile::tempdir().unwrap();

    // Pre-set remotes.json (pointing to local snippet server) and fetch once to write cache.
    let remote = crate::remote::RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "rules".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: pp_script::ScriptDialect::QuantumultX,
        ..RemoteResource::default()
    }];
    remote.save(&remotes).unwrap();
    let report = remote.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1, "snippet fetch should succeed");

    let mut cfg = test_config(&dir, base);
    cfg.mitm_enabled = true;
    cfg.save().unwrap();
    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    state.start().await.unwrap();

    // MITM is running, and scheduler holds remote snippet task scripts.
    let status = state.status().await;
    assert!(status.mitm_addr.is_some(), "MITM should be running");
    let tasks = state
        .scheduler()
        .expect("scheduler should run")
        .list_tasks();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].name, "每日签到");

    // stop normally: reverse order shutdown system proxy → MITM → scheduler → core.
    state.stop().await;
    let status = state.status().await;
    assert!(status.mitm_addr.is_none());
    assert!(!status.core_running);
}

/// core_type=Mihomo: fetch clash subscription + mihomo config composition, fake core starts successfully.
#[tokio::test]
async fn start_with_mihomo_core_fetches_clash_and_starts() {
    let yaml = "port: 7890\nproxies:\n  - name: n1\n    type: direct\nrules:\n  - MATCH,DIRECT\n";
    let addr = spawn_server(StatusCode::OK, yaml).await;
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = test_config(&dir, format!("http://{addr}"));
    cfg.core_type = CoreType::Mihomo;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    state.start().await.unwrap();

    let status = state.status().await;
    assert!(status.core_running, "mihomo core should start");
    assert_eq!(status.rule_mode, "rule", "default rule mode is rule");
    // yaml contains 1 `MATCH,DIRECT` rule; clash_api is disabled by default → no API address.
    assert_eq!(status.rule_count, 1);
    assert_eq!(status.clash_api_url, None);

    state.stop().await;
    let status = state.status().await;
    assert!(!status.core_running);
}

/// Rule mode + Clash API integration: startup succeeds and when clash_api_enabled, best-effort push persisted mode via
/// `PATCH /configs` (for sing-box this is the only effective channel); status()
/// returns rule_mode / rule_count / clash_api_url new fields.
#[tokio::test]
async fn start_pushes_rule_mode_via_clash_api_when_enabled() {
    // Fake Clash API server: receives PATCH /configs and records request body.
    let captured = Arc::new(std::sync::Mutex::new(None));
    let captured_ref = Arc::clone(&captured);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let clash_addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/configs",
            axum::routing::patch(
                move |req: axum::http::Request<axum::body::Body>| async move {
                    let bytes = axum::body::to_bytes(req.into_body(), 1024).await.unwrap();
                    *captured_ref.lock().unwrap() = Some(bytes.to_vec());
                    axum::http::StatusCode::NO_CONTENT
                },
            ),
        )
        // Readiness probe used by wait_clash_api_ready before the PATCH.
        .route(
            "/version",
            axum::routing::get(|| async { axum::http::StatusCode::OK }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Subscription contains 1 route rule (for rule_count assertion).
    let sub_body = r#"{
            "outbounds": [{ "type": "direct", "tag": "direct" }],
            "route": { "final": "direct", "rules": [{"action": "sniff"}] }
        }"#;
    let addr = spawn_server(StatusCode::OK, sub_body).await;
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = test_config(&dir, format!("http://{addr}"));
    cfg.clash_api_enabled = true;
    cfg.clash_api_port = clash_addr.port();
    cfg.clash_api_secret = "sekret".to_string();
    cfg.rule_mode = "global".to_string();
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    state.start().await.unwrap();

    // Push persisted mode via Clash API during startup (rule_mode=global).
    let body = captured
        .lock()
        .unwrap()
        .clone()
        .expect("should receive PATCH request");
    assert_eq!(body, br#"{"mode":"global"}"#);

    // Status extension fields.
    let status = state.status().await;
    assert_eq!(status.rule_mode, "global");
    assert_eq!(status.rule_count, 1);
    assert_eq!(
        status.clash_api_url,
        Some(format!("http://127.0.0.1:{}", clash_addr.port()))
    );

    // After stop, rule count is cleared and API address disappears.
    state.stop().await;
    let status = state.status().await;
    assert_eq!(status.rule_count, 0);
    assert_eq!(status.clash_api_url, None);
}

/// Item 1: clash format subscription + sing-box core → start returns clear format/core mismatch error,
/// core does not start, system proxy zero calls.
#[tokio::test]
async fn start_rejects_clash_format_with_singbox_core() {
    const YAML: &str = "port: 7890\nproxies:\n  - name: n1\n    type: ss\n    server: example.com\n    port: 8388\n    cipher: aes-256-gcm\n    password: pw\nrules:\n  - MATCH,DIRECT\n";
    let addr = spawn_server(StatusCode::OK, YAML).await;
    let dir = tempfile::tempdir().unwrap();
    // Generic subscription path: subscriptions.json points to local server (clash format),
    // client.json selects this subscription (new model: selected subscription is the only effective one).
    let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("clash-sub", &format!("http://{addr}/sub"), true, None)
        .unwrap();

    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        fake_core_script(&dir),
    );
    cfg.active_subscription_id = Some(sub.id);
    cfg.mitm_enabled = false;
    cfg.system_proxy_enabled = true;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    let err = state.start().await.unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("clash"),
        "should contain detected format clash: {msg}"
    );
    assert!(
        msg.contains("mihomo"),
        "should contain supported core mihomo: {msg}"
    );
    assert!(
        msg.contains("切换核心类型"),
        "should prompt to switch core type: {msg}"
    );

    // Core not started, system proxy zero calls.
    let status = state.status().await;
    assert!(!status.core_running);
    assert_eq!(mock.calls(), vec![]);
}

/// Generic subscription integration: subscriptions.json points to local server (base64 share links) → start succeeds.
#[tokio::test]
async fn start_with_subscription_store_fetches_and_starts() {
    // Local server returns base64-encoded vless share link (no external network).
    let link = "vless://12345678-1234-1234-1234-123456789012@example.com:443?security=tls&sni=example.com#n1";
    let body = base64::engine::general_purpose::STANDARD.encode(link);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    tokio::spawn(async move {
        let app = axum::Router::new().route(
            "/sub",
            axum::routing::get(move || async move {
                (
                    [(
                        "subscription-userinfo",
                        "upload=1; download=2; total=100; expire=3",
                    )],
                    body,
                )
            }),
        );
        axum::serve(listener, app).await.unwrap();
    });

    let dir = tempfile::tempdir().unwrap();
    // subscriptions.json points to local server, client.json selects this subscription.
    let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("local", &format!("{base}/sub"), true, None)
        .unwrap();

    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        fake_core_script(&dir),
    );
    cfg.active_subscription_id = Some(sub.id);
    cfg.mitm_enabled = false;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    state.start().await.unwrap();

    let status = state.status().await;
    assert!(
        status.core_running,
        "generic subscription path core should start"
    );

    state.stop().await;
    let status = state.status().await;
    assert!(!status.core_running);
}

#[tokio::test]
async fn start_with_mitm_chain_runs_mitm_before_core_and_proxy_points_at_main_port() {
    let base = spawn_integration_server().await;
    let dir = tempfile::tempdir().unwrap();

    // Pre-set remote snippet cache (contains MITM whitelist *.example.com / api.example2.com).
    let remote = crate::remote::RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "rules".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: pp_script::ScriptDialect::QuantumultX,
        ..RemoteResource::default()
    }];
    remote.save(&remotes).unwrap();
    let report = remote.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1, "snippet fetch should succeed");

    // Fake core copies received composed config out, for asserting core actual config.
    let capture = dir.path().join("core-config-capture.json");
    let core_bin = fake_core_capturing_args(&dir, &capture);
    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        base,
        "tok",
        CoreType::SingBox,
        core_bin,
    );
    cfg.mitm_enabled = true;
    cfg.system_proxy_enabled = true;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    state.start().await.unwrap();

    let status = state.status().await;
    let mitm_addr = status.mitm_addr.expect("MITM should be running");

    // System proxy points to core mixed main port (not MITM random port).
    let calls = mock.calls();
    assert_eq!(calls.len(), 1, "system proxy should only be enabled once");
    match &calls[0] {
        SysProxyCall::Enable(addr) => {
            assert_eq!(
                addr.port(),
                17890,
                "system proxy should point to core mixed main entry port"
            )
        }
        SysProxyCall::Disable => panic!("should not appear disable"),
    }

    // MITM starts before core: core received config's pp-mitm outbound port == MITM actual
    // listen port (random port, only known after MITM starts).
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

    // Dual mixed inbounds: main entry + return entry.
    let inbounds = core_config["inbounds"].as_array().unwrap();
    assert_eq!(inbounds.len(), 2);
    assert_eq!(inbounds[0]["tag"], "main-in");
    assert_eq!(inbounds[0]["listen_port"], 17890);
    assert_eq!(inbounds[1]["tag"], "mitm-return");
    assert_eq!(inbounds[1]["listen_port"], 17891);

    // pp-mitm outbound points to MITM actual listen port.
    let outbounds = core_config["outbounds"].as_array().unwrap();
    let pp_mitm = outbounds
        .iter()
        .find(|o| o["tag"] == "pp-mitm")
        .expect("core config should contain pp-mitm outbound");
    assert_eq!(pp_mitm["type"], "http");
    assert_eq!(pp_mitm["server_port"], mitm_addr.port());

    // Whitelist routing rule: inbound matches main entry, domains are correctly split by wildcard/exact.
    let rules = core_config["route"]["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["inbound"], serde_json::json!(["main-in"]));
    assert_eq!(
        rules[0]["domain_suffix"],
        serde_json::json!(["example.com"])
    );
    assert_eq!(rules[0]["domain"], serde_json::json!(["api.example2.com"]));
    assert_eq!(rules[0]["outbound"], "pp-mitm");

    // Running status extension: composed config contains 1 MITM whitelist routing rule.
    let status = state.status().await;
    assert_eq!(status.rule_count, 1);

    state.stop().await;
    let status = state.status().await;
    assert!(status.mitm_addr.is_none());
    assert!(!status.core_running);
}

/// Profile layer integration: subscription (2 nodes + selector/direct groups) goes through template grouping, subscription-associated
/// override template JS override takes effect, compose normally injects inbounds and MITM chain.
#[tokio::test]
async fn start_with_profile_applies_template_groups_and_js_override() {
    let body = r#"{
            "log": {"level": "debug"},
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } },
                { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
                  "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
                { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
                { "type": "direct", "tag": "direct" }
            ],
            "route": { "final": "direct" }
        }"#;
    let addr = spawn_server(StatusCode::OK, body).await;
    let dir = tempfile::tempdir().unwrap();

    // Pre-set profiles.json: a SingBox template whose js_override modifies dns.strategy.
    let profile_id = uuid::Uuid::new_v4();
    crate::profile::ProfileStoreV2::new(dir.path().to_path_buf())
        .save(&[crate::profile::Profile {
            id: profile_id,
            name: "Default".to_string(),
            core_type: pp_common::CoreType::SingBox,
            yaml_override: String::new(),
            js_override: r#"function main(c) { c.dns.strategy = "ipv4_only"; return c; }"#
                .to_string(),
            yaml_url: None,
            js_url: None,
        }])
        .unwrap();

    // Subscription is associated with this template (pure association): subscriptions.json points to local server and is associated
    // with profile_id, client.json selects this subscription.
    let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("local", &format!("http://{addr}/sub"), true, None)
        .unwrap();
    store.set_profile_id(sub.id, Some(profile_id)).unwrap();

    let capture = dir.path().join("core-config-capture.json");
    let core_bin = fake_core_capturing_args(&dir, &capture);
    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        core_bin,
    );
    cfg.active_subscription_id = Some(sub.id);
    cfg.mitm_enabled = true;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    state.start().await.unwrap();

    let status = state.status().await;
    let mitm_addr = status.mitm_addr.expect("MITM should be running");

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

    // Nodes grouped by template: n1/n2 retained and referenced by proxy (select) / auto (url-test) groups.
    let outbounds = core_config["outbounds"].as_array().unwrap();
    assert!(outbounds.iter().any(|o| o["tag"] == "n1"));
    assert!(outbounds.iter().any(|o| o["tag"] == "n2"));
    let proxy = outbounds.iter().find(|o| o["tag"] == "proxy").unwrap();
    assert_eq!(proxy["type"], "selector");
    let proxy_out: Vec<&str> = proxy["outbounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(proxy_out.contains(&"n1") && proxy_out.contains(&"n2"));
    let auto = outbounds.iter().find(|o| o["tag"] == "auto").unwrap();
    assert_eq!(auto["type"], "urltest");

    // Template replaces subscription's own log/route, JS override takes effect.
    assert_eq!(core_config["log"]["level"], "info");
    assert_eq!(core_config["dns"]["strategy"], "ipv4_only");
    assert_eq!(core_config["route"]["final"], "proxy");

    // compose injects inbounds and MITM chain.
    let inbounds = core_config["inbounds"].as_array().unwrap();
    assert_eq!(inbounds.len(), 2);
    assert_eq!(inbounds[0]["tag"], "main-in");
    assert_eq!(inbounds[1]["tag"], "mitm-return");
    let pp_mitm = outbounds.iter().find(|o| o["tag"] == "pp-mitm").unwrap();
    assert_eq!(pp_mitm["type"], "http");
    assert_eq!(pp_mitm["server_port"], mitm_addr.port());

    state.stop().await;
}

/// Subscription-associated override template has illegal JS override → start returns Err; core not started, system proxy zero calls.
#[tokio::test]
async fn start_rolls_back_on_invalid_profile_js_override() {
    let body = r#"{
            "outbounds": [
                { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
                  "uuid": "12345678-1234-1234-1234-123456789012",
                  "tls": { "enabled": true, "server_name": "example.com" } }
            ]
        }"#;
    let addr = spawn_server(StatusCode::OK, body).await;
    let dir = tempfile::tempdir().unwrap();

    // Pre-set illegal JS override (unclosed parenthesis) template, and associate with selected subscription (pure association).
    let profile_id = uuid::Uuid::new_v4();
    crate::profile::ProfileStoreV2::new(dir.path().to_path_buf())
        .save(&[crate::profile::Profile {
            id: profile_id,
            name: "Default".to_string(),
            core_type: pp_common::CoreType::SingBox,
            yaml_override: String::new(),
            js_override: "function main(c) { return c;".to_string(),
            yaml_url: None,
            js_url: None,
        }])
        .unwrap();
    let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("local", &format!("http://{addr}/sub"), true, None)
        .unwrap();
    store.set_profile_id(sub.id, Some(profile_id)).unwrap();

    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        fake_core_script(&dir),
    );
    cfg.active_subscription_id = Some(sub.id);
    cfg.mitm_enabled = true;
    cfg.system_proxy_enabled = true;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    assert!(state.start().await.is_err());

    // Profile build failure occurs before any component starts: core not started, MITM not started,
    // system proxy zero calls.
    let status = state.status().await;
    assert!(!status.core_running);
    assert!(status.mitm_addr.is_none());
    assert_eq!(mock.calls(), vec![]);
}

/// New model: no subscription selected and no legacy Hub config → start returns "Please select a subscription to use on the home page first".
#[tokio::test]
async fn start_requires_active_subscription_selection() {
    let dir = tempfile::tempdir().unwrap();
    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        fake_core_script(&dir),
    );
    cfg.mitm_enabled = false;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    let err = state.start().await.unwrap_err();
    assert!(err.to_string().contains("请先在首页选择"), "{err}");
    assert_eq!(mock.calls(), vec![]);
}

/// New model: active_subscription_id points to disabled subscription → clear error, core does not start.
#[tokio::test]
async fn start_rejects_disabled_selected_subscription() {
    let addr = spawn_server(StatusCode::OK, "{}").await;
    let dir = tempfile::tempdir().unwrap();
    let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("off", &format!("http://{addr}/sub"), false, None)
        .unwrap();

    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        fake_core_script(&dir),
    );
    cfg.active_subscription_id = Some(sub.id);
    cfg.mitm_enabled = false;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    let err = state.start().await.unwrap_err();
    assert!(err.to_string().contains("已停用"), "{err}");
    assert_eq!(mock.calls(), vec![]);
}

/// New model: subscription-associated override template core type does not match current core → clear error (contains sing-box /
/// mihomo display names), core does not start.
#[tokio::test]
async fn start_rejects_profile_core_type_mismatch() {
    let body = r#"{ "outbounds": [] }"#;
    let addr = spawn_server(StatusCode::OK, body).await;
    let dir = tempfile::tempdir().unwrap();

    let profile_id = uuid::Uuid::new_v4();
    crate::profile::ProfileStoreV2::new(dir.path().to_path_buf())
        .save(&[crate::profile::Profile {
            id: profile_id,
            name: "mihomo template".to_string(),
            core_type: pp_common::CoreType::Mihomo,
            yaml_override: String::new(),
            js_override: String::new(),
            yaml_url: None,
            js_url: None,
        }])
        .unwrap();
    let store = subscription::SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("local", &format!("http://{addr}/sub"), true, None)
        .unwrap();
    store.set_profile_id(sub.id, Some(profile_id)).unwrap();

    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        CoreType::SingBox,
        fake_core_script(&dir),
    );
    cfg.active_subscription_id = Some(sub.id);
    cfg.mitm_enabled = false;
    cfg.save().unwrap();

    let mock = Arc::new(MockSystemProxy::new());
    let mut state = ClientState::with_system_proxy(cfg, mock.clone());
    let err = state.start().await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("不匹配"), "{msg}");
    assert!(msg.contains("sing-box") && msg.contains("mihomo"), "{msg}");
    assert_eq!(mock.calls(), vec![]);
}

/// Injected custom Notifier reaches ScriptHost: manually run task containing `$notify` and it is recorded.
#[tokio::test]
async fn injected_notifier_receives_task_notify() {
    let base = spawn_integration_server().await;
    let dir = tempfile::tempdir().unwrap();

    // Pre-set remotes.json and fetch once to write cache (task.js contains $notify).
    let remote = crate::remote::RemoteManager::new(dir.path().to_path_buf());
    let remotes = vec![RemoteResource {
        name: "rules".into(),
        url: format!("{base}/snippet"),
        kind: RemoteKind::Snippet,
        dialect: pp_script::ScriptDialect::QuantumultX,
        ..RemoteResource::default()
    }];
    remote.save(&remotes).unwrap();
    let report = remote.fetch_all(&remotes).await;
    assert_eq!(report.fetched, 1, "snippet fetch should succeed");

    let mut cfg = test_config(&dir, base);
    cfg.mitm_enabled = true;
    cfg.save().unwrap();
    let notifier = Arc::new(RecordingNotifier::new());
    let mut state = ClientState::with_notifier(cfg, notifier.clone());
    state.start().await.unwrap();

    // Manually run task containing $notify, verify notification reaches injected notifier.
    let scheduler = state.scheduler_handle().expect("scheduler should run");
    let out = scheduler.run_now("每日签到").await.unwrap();
    assert_eq!(out.0["code"], 0);
    let calls = notifier.calls();
    assert_eq!(
        calls.len(),
        1,
        "task $notify should trigger one notification"
    );
    assert_eq!(calls[0].0, "签到成功");

    state.stop().await;
}
