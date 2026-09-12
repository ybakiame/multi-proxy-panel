use super::*;
use pp_client::local_override::{
    CoreLocalOverride, LocalOverride, LocalOverrideStore, LocalRule, RuleAction,
    RuleAdvancedOptions, RuleMatchType,
};
use pp_client::{CachedSubscriptionContent, SubFormat};
use std::io::{Read, Write};

struct TestDir(PathBuf);

impl TestDir {
    fn new() -> Self {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "pp-client-ui-preview-test-{}-{}",
            std::process::id(),
            n
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spawn_sub_server(body: &'static str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else {
                continue;
            };
            let mut buf = [0u8; 8192];
            let _ = stream.read(&mut buf);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

const PREVIEW_SUB_JSON: &str = r#"{
    "outbounds": [
        { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
          "uuid": "12345678-1234-1234-1234-123456789012",
          "tls": { "enabled": true, "server_name": "example.com" } }
    ]
}"#;

#[tokio::test]
async fn preview_core_config_specified_subscription_generates_config() {
    let dir = TestDir::new();
    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.save().unwrap();

    let base = spawn_sub_server(PREVIEW_SUB_JSON);
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("spec", &format!("{base}/sub"), false, None)
        .unwrap();

    let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
        .await
        .expect("specified subscription preview should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("sing-box preview should be JSON");
    assert!(value.get("outbounds").is_some());
    assert!(value.get("inbounds").is_some());
}

#[tokio::test]
async fn preview_core_config_specified_unknown_subscription_errors() {
    let dir = TestDir::new();
    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.save().unwrap();

    let err = preview_core_config_impl(dir.path().to_path_buf(), Some(uuid::Uuid::new_v4()))
        .await
        .unwrap_err();
    assert!(err.contains("订阅不存在"), "{err}");
}

#[tokio::test]
async fn preview_core_config_none_uses_active_subscription_selection() {
    let dir = TestDir::new();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let off = store
        .add("off", "https://example.com/sub", false, None)
        .unwrap();
    let mut cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.active_subscription_id = Some(off.id);
    cfg.save().unwrap();

    let err = preview_core_config_impl(dir.path().to_path_buf(), None)
        .await
        .unwrap_err();
    assert!(err.contains("已停用"), "{err}");

    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.save().unwrap();
    let err = preview_core_config_impl(dir.path().to_path_buf(), None)
        .await
        .unwrap_err();
    assert!(err.contains("请先在首页选择要使用的订阅"), "{err}");
}

#[tokio::test]
async fn preview_core_config_specified_uses_local_cache_without_network() {
    let dir = TestDir::new();
    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.save().unwrap();

    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("spec", "http://127.0.0.1:1/unreachable", false, None)
        .unwrap();
    store
        .write_cached_content(
            sub.id,
            &CachedSubscriptionContent {
                format: SubFormat::SingBoxJson,
                singbox_nodes: vec![serde_json::json!({
                    "type": "vless",
                    "tag": "n1",
                    "server": "example.com",
                    "server_port": 443,
                    "uuid": "12345678-1234-1234-1234-123456789012",
                    "tls": { "enabled": true, "server_name": "example.com" },
                })],
            },
        )
        .unwrap();

    let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
        .await
        .expect("cached preview should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("sing-box preview should be JSON");
    assert!(value.get("outbounds").is_some());
}

#[tokio::test]
async fn preview_core_config_injects_active_local_override_rules() {
    let dir = TestDir::new();
    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.save().unwrap();

    // Cache the subscription so preview does not touch the network.
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("spec", "http://127.0.0.1:1/unreachable", false, None)
        .unwrap();
    store
        .write_cached_content(
            sub.id,
            &CachedSubscriptionContent {
                format: SubFormat::SingBoxJson,
                singbox_nodes: vec![serde_json::json!({
                    "type": "vless",
                    "tag": "n1",
                    "server": "example.com",
                    "server_port": 443,
                    "uuid": "12345678-1234-1234-1234-123456789012",
                    "tls": { "enabled": true, "server_name": "example.com" },
                })],
            },
        )
        .unwrap();

    // One enabled local rule → injected by `inject_local_override_warn_only`
    // (scenario templates removed: all enabled rules are injected).
    let rule_id = "r-local".to_string();
    let ovr = LocalOverride {
        singbox: CoreLocalOverride {
            rules: vec![LocalRule {
                id: rule_id,
                name: "local reject".to_string(),
                enabled: true,
                match_type: RuleMatchType::DomainSuffix,
                target: "preview-local.example".to_string(),
                action: RuleAction::Reject,
                advanced: RuleAdvancedOptions::default(),
                note: String::new(),
                created_at: 0,
                sort_order: 0,
            }],
            rule_sets: Vec::new(),
            enabled: true,
        },
        ..Default::default()
    };
    LocalOverrideStore::new(dir.path().to_path_buf())
        .save(&ovr)
        .unwrap();

    let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
        .await
        .expect("preview with local override should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("sing-box preview should be JSON");
    let rules = value["route"]["rules"]
        .as_array()
        .expect("preview should contain route.rules");
    assert!(
        rules.iter().any(|r| {
            r["domain_suffix"] == "preview-local.example" && r["outbound"] == "reject"
        }),
        "preview should inject the active local override rule: {rules:?}"
    );
}

#[tokio::test]
async fn preview_core_config_specified_fallback_writes_cache() {
    let dir = TestDir::new();
    let cfg = ClientConfig::new(
        dir.path().to_path_buf(),
        String::new(),
        String::new(),
        PathBuf::new(),
    );
    cfg.save().unwrap();

    let base = spawn_sub_server(PREVIEW_SUB_JSON);
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("spec", &format!("{base}/sub"), false, None)
        .unwrap();

    let text = preview_core_config_impl(dir.path().to_path_buf(), Some(sub.id))
        .await
        .expect("fallback fetch preview should succeed");
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("sing-box preview should be JSON");
    assert!(value.get("outbounds").is_some());
    let cached = store
        .load_cached_content(sub.id)
        .expect("fallback should write cache");
    assert_eq!(cached.format, SubFormat::SingBoxJson);
    assert!(!cached.singbox_nodes.is_empty());
}
