#[cfg(test)]
use base64::Engine as _;

use super::*;

const SUB_JSON: &str = r#"{
    "log": { "level": "info" },
    "outbounds": [
        { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443,
          "uuid": "12345678-1234-1234-1234-123456789012",
          "tls": { "enabled": true, "server_name": "example.com" } },
        { "type": "hysteria2", "tag": "n2", "server": "example.org", "server_port": 8443,
          "password": "pw", "tls": { "enabled": true, "server_name": "example.org" } },
        { "type": "selector", "tag": "proxy", "outbounds": ["n1"] },
        { "type": "direct", "tag": "direct" }
    ],
    "route": { "final": "n1" }
}"#;

const SUB_YAML: &str = "port: 7890\nproxies:\n  - name: n1\n    type: vless\n    server: example.com\n    port: 443\n    uuid: 12345678-1234-1234-1234-123456789012\n  - name: n2\n    type: ss\n    server: example.org\n    port: 8388\n    cipher: aes-256-gcm\n    password: pw\nrules:\n  - MATCH,DIRECT\n";

const SHARE_LINKS: &str = "ss://YWVzLTI1Ni1nY206cGFzc3dvcmQ@example.com:8388#ss-node\nvless://12345678-1234-1234-1234-123456789012@example.com:443?security=tls&sni=example.com#vless-node\n";

fn b64(s: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(s)
}

async fn spawn_server(app: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

// ---------- Legacy Hub path regression ----------

#[tokio::test]
async fn fetch_singbox_config_parses_config_and_info() {
    let app = axum::Router::new().route(
        "/sub/{token}",
        axum::routing::get(|| async {
            (
                [(
                    "subscription-userinfo",
                    "upload=100; download=200; total=1000; expire=1700000000",
                )],
                SUB_JSON,
            )
        }),
    );
    let base = spawn_server(app).await;

    let fetcher = SubscriptionFetcher::new();
    let (config, info) = fetcher.fetch_singbox_config(&base, "tok").await.unwrap();

    assert_eq!(config["outbounds"][0]["tag"], "n1");
    assert_eq!(config["route"]["final"], "n1");

    let info = info.unwrap();
    assert_eq!(info.upload, Some(100));
    assert_eq!(info.download, Some(200));
    assert_eq!(info.total, Some(1000));
    assert_eq!(info.expire, Some(1700000000));
}

#[tokio::test]
async fn fetch_singbox_config_returns_client_error_on_4xx() {
    let app = axum::Router::new().route(
        "/sub/{token}",
        axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
    );
    let base = spawn_server(app).await;

    let fetcher = SubscriptionFetcher::new();
    let err = fetcher
        .fetch_singbox_config(&base, "missing")
        .await
        .unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));
}

#[tokio::test]
async fn fetch_clash_config_returns_yaml_text_and_info() {
    let app = axum::Router::new().route(
        "/sub/{token}",
        axum::routing::get(|| async {
            (
                [(
                    "subscription-userinfo",
                    "upload=100; download=200; total=1000; expire=1700000000",
                )],
                SUB_YAML,
            )
        }),
    );
    let base = spawn_server(app).await;

    let fetcher = SubscriptionFetcher::new();
    let (yaml, info) = fetcher.fetch_clash_config(&base, "tok").await.unwrap();

    // YAML text returned as-is.
    assert_eq!(yaml, SUB_YAML);
    assert!(yaml.contains("proxies:"));

    let info = info.unwrap();
    assert_eq!(info.upload, Some(100));
    assert_eq!(info.download, Some(200));
    assert_eq!(info.total, Some(1000));
    assert_eq!(info.expire, Some(1700000000));
}

#[tokio::test]
async fn fetch_clash_config_returns_client_error_on_4xx() {
    let app = axum::Router::new().route(
        "/sub/{token}",
        axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
    );
    let base = spawn_server(app).await;

    let fetcher = SubscriptionFetcher::new();
    let err = fetcher
        .fetch_clash_config(&base, "missing")
        .await
        .unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));
}

#[test]
fn parse_userinfo_ignores_malformed_pairs() {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HeaderName::from_static("subscription-userinfo"),
        reqwest::header::HeaderValue::from_static("upload=1; bad; total=1024; expire=5"),
    );
    let info = parse_subscription_userinfo(&headers).unwrap();
    assert_eq!(info.upload, Some(1));
    assert_eq!(info.total, Some(1024));
    assert_eq!(info.expire, Some(5));
    assert_eq!(info.download, None);
}

// ---------- Four format sniffing cases ----------

#[test]
fn sniff_base64_share_links() {
    let body = b64(SHARE_LINKS);
    let result = parse_subscription_body(&body, None).unwrap();
    assert_eq!(result.format, SubFormat::ShareLinks);
    assert_eq!(result.singbox_nodes.len(), 2);
    assert_eq!(result.mihomo_nodes.len(), 2);
    assert_eq!(result.singbox_nodes[0]["type"], "shadowsocks");
    assert_eq!(result.singbox_nodes[1]["type"], "vless");
}

#[test]
fn sniff_clash_yaml() {
    let result = parse_subscription_body(SUB_YAML, None).unwrap();
    assert_eq!(result.format, SubFormat::ClashYaml);
    assert_eq!(result.mihomo_nodes.len(), 2);
    assert_eq!(result.mihomo_nodes[0]["type"], "vless");
    assert_eq!(result.singbox_nodes.len(), 2);
    assert_eq!(result.singbox_nodes[0]["type"], "vless");
    assert_eq!(result.singbox_nodes[1]["type"], "shadowsocks");
    assert_eq!(result.singbox_nodes[1]["method"], "aes-256-gcm");
}

#[test]
fn sniff_singbox_json() {
    let result = parse_subscription_body(SUB_JSON, None).unwrap();
    assert_eq!(result.format, SubFormat::SingBoxJson);
    assert_eq!(result.singbox_nodes.len(), 2);
    assert_eq!(result.singbox_nodes[0]["tag"], "n1");
    assert_eq!(result.mihomo_nodes.len(), 2);
    assert_eq!(result.mihomo_nodes[0]["name"], "n1");
    assert_eq!(result.mihomo_nodes[0]["type"], "vless");
}

#[test]
fn sniff_plaintext_share_links() {
    let result = parse_subscription_body(SHARE_LINKS, None).unwrap();
    assert_eq!(result.format, SubFormat::ShareLinks);
    assert_eq!(result.singbox_nodes.len(), 2);
    assert_eq!(result.mihomo_nodes.len(), 2);
    assert!(result.warnings.is_empty());
}

#[test]
fn sniff_rejects_unrecognized_content() {
    let err = parse_subscription_body("just some plain text", None).unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));
    let err = parse_subscription_body("  ", None).unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));
}

#[test]
fn sniff_unsupported_clash_proxy_is_skipped_with_warning() {
    let yaml = "proxies:\n  - name: ok\n    type: vless\n    server: a.com\n    port: 443\n    uuid: 12345678-1234-1234-1234-123456789012\n  - name: bad\n    type: wireguard\n    server: b.com\n    port: 51820\n";
    let result = parse_subscription_body(yaml, None).unwrap();
    assert_eq!(
        result.mihomo_nodes.len(),
        2,
        "mihomo side keeps original proxies"
    );
    assert_eq!(
        result.singbox_nodes.len(),
        1,
        "sing-box side skips unsupported type"
    );
    assert_eq!(result.singbox_nodes[0]["tag"], "ok");
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("bad"));
}

// ---------- fetch_subscription e2e (local axum, no external network) ----------

#[tokio::test]
async fn fetch_subscription_gets_base64_links_and_userinfo() {
    let body = b64(SHARE_LINKS);
    let app = axum::Router::new().route(
        "/sub",
        axum::routing::get(move || async move {
            (
                [(
                    "subscription-userinfo",
                    "upload=10; download=20; total=1000; expire=1700000000",
                )],
                body,
            )
        }),
    );
    let base = spawn_server(app).await;

    let result = fetch_subscription(&format!("{base}/sub")).await.unwrap();
    assert_eq!(result.format, SubFormat::ShareLinks);
    assert_eq!(result.singbox_nodes.len(), 2);
    let info = result.userinfo.unwrap();
    assert_eq!(info.upload, Some(10));
    assert_eq!(info.download, Some(20));
}

#[tokio::test]
async fn fetch_subscription_returns_client_error_on_4xx() {
    let app = axum::Router::new().route(
        "/sub",
        axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
    );
    let base = spawn_server(app).await;
    let err = fetch_subscription(&format!("{base}/sub"))
        .await
        .unwrap_err();
    assert!(matches!(err, PanelError::Client(_)));
}

// ---------- UA override: custom UA and default clash.meta ----------

#[tokio::test]
async fn fetch_subscription_sends_custom_or_default_user_agent() {
    let body = b64(SHARE_LINKS);
    let uas = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let app = {
        let uas = std::sync::Arc::clone(&uas);
        axum::Router::new().route(
            "/sub",
            axum::routing::get(move |headers: axum::http::HeaderMap| {
                let uas = std::sync::Arc::clone(&uas);
                async move {
                    uas.lock().unwrap().push(
                        headers
                            .get("user-agent")
                            .and_then(|v| v.to_str().ok())
                            .unwrap_or_default()
                            .to_string(),
                    );
                    body.clone()
                }
            }),
        )
    };
    let base = spawn_server(app).await;

    // Custom UA passes through.
    let result = fetch_subscription_with_ua(&format!("{base}/sub"), Some("clash-verge/0.6.5"))
        .await
        .unwrap();
    assert_eq!(result.format, SubFormat::ShareLinks);
    assert_eq!(result.singbox_nodes.len(), 2);

    // Default (None) and empty string both fallback to clash.meta.
    let result = fetch_subscription(&format!("{base}/sub")).await.unwrap();
    assert_eq!(result.format, SubFormat::ShareLinks);
    let result = fetch_subscription_with_ua(&format!("{base}/sub"), Some("  "))
        .await
        .unwrap();
    assert_eq!(result.format, SubFormat::ShareLinks);

    let got = uas.lock().unwrap();
    assert_eq!(
        got.as_slice(),
        ["clash-verge/0.6.5", "clash.meta", "clash.meta"]
    );
}

// ---------- SubscriptionStore CRUD ----------

#[test]
fn subscription_store_crud() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());

    // Missing file → empty list.
    assert!(store.load().unwrap().is_empty());
    assert!(!store.file().exists());

    // add.
    let sub1 = store
        .add("sub-a", "https://example.com/sub", true, None)
        .unwrap();
    let sub2 = store
        .add(
            "sub-b",
            "https://example.org/sub",
            false,
            Some("clash-verge"),
        )
        .unwrap();
    let mut all = store.load().unwrap();
    assert_eq!(all.len(), 2);
    assert_ne!(sub1.id, sub2.id);
    assert!(
        all.iter().all(|s| s.profile_id.is_none()),
        "new subscription defaults to no override"
    );

    // set_enabled.
    store.set_enabled(sub1.id, false).unwrap();
    all = store.load().unwrap();
    assert!(!all.iter().find(|s| s.id == sub1.id).unwrap().enabled);

    // set_profile_id: associate override template and persist; None cancels
    // association; non-existent id errors.
    let profile_id = Uuid::new_v4();
    store.set_profile_id(sub1.id, Some(profile_id)).unwrap();
    all = store.load().unwrap();
    assert_eq!(
        all.iter().find(|s| s.id == sub1.id).unwrap().profile_id,
        Some(profile_id)
    );
    store.set_profile_id(sub1.id, None).unwrap();
    all = store.load().unwrap();
    assert_eq!(
        all.iter().find(|s| s.id == sub1.id).unwrap().profile_id,
        None
    );
    assert!(
        store
            .set_profile_id(Uuid::new_v4(), Some(profile_id))
            .is_err()
    );

    // remove.
    store.remove(sub1.id).unwrap();
    all = store.load().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, sub2.id);

    // Silently handles non-existent id.
    store.remove(Uuid::new_v4()).unwrap();
    store.set_enabled(Uuid::new_v4(), true).unwrap();
    assert_eq!(store.load().unwrap().len(), 1);
}

#[test]
fn subscription_store_tolerates_corrupted_file() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    std::fs::write(store.file(), "{ not json").unwrap();
    assert!(store.load().unwrap().is_empty());
}

/// UA persistence + legacy subscriptions.json (without user_agent field)
/// compatibility.
#[test]
fn subscription_store_persists_user_agent_and_tolerates_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());

    let sub = store
        .add("sub", "https://example.com/sub", true, Some("sing-box"))
        .unwrap();
    assert_eq!(sub.user_agent.as_deref(), Some("sing-box"));
    let loaded = store.load().unwrap();
    assert_eq!(loaded[0].user_agent.as_deref(), Some("sing-box"));

    // Legacy file without user_agent / profile_id fields → deserialize as
    // None.
    std::fs::write(
        store.file(),
        r#"[{"id":"00000000-0000-0000-0000-000000000001","name":"old","url":"https://x.com/sub","enabled":true,"node_count":0}]"#,
    )
    .unwrap();
    let legacy = store.load().unwrap();
    assert_eq!(legacy[0].user_agent, None);
    assert_eq!(legacy[0].profile_id, None);
}

/// update: name / url / user_agent update and persist; URL change clears
/// userinfo cache, URL unchanged retains cache.
#[test]
fn subscription_store_update_changes_fields_and_clears_userinfo_on_url_change() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("sub", "https://example.com/a", true, Some("ua"))
        .unwrap();
    // Associate an override template (update should not clear this
    // association).
    let profile_id = Uuid::new_v4();
    store.set_profile_id(sub.id, Some(profile_id)).unwrap();
    // Simulate a successful fetch: write userinfo and node_count.
    let mut subs = store.load().unwrap();
    let s = subs.iter_mut().find(|s| s.id == sub.id).unwrap();
    s.userinfo = Some(SubscriptionInfo {
        upload: Some(1),
        download: Some(2),
        total: Some(100),
        expire: None,
    });
    s.node_count = 5;
    store.save(&subs).unwrap();

    // URL change → userinfo / node_count cleared, name / user_agent updated.
    store
        .update(sub.id, "new-name", "https://example.com/b", Some("new-ua"))
        .unwrap();
    let subs = store.load().unwrap();
    let s = subs.iter().find(|s| s.id == sub.id).unwrap();
    assert_eq!(s.name, "new-name");
    assert_eq!(s.url, "https://example.com/b");
    assert_eq!(s.user_agent.as_deref(), Some("new-ua"));
    assert_eq!(s.userinfo, None);
    assert_eq!(s.node_count, 0);
    assert_eq!(
        s.profile_id,
        Some(profile_id),
        "update does not change subscription association"
    );

    // URL unchanged → cache retained (only name changed).
    store
        .update(sub.id, "renamed", "https://example.com/b", None)
        .unwrap();
    let subs = store.load().unwrap();
    let s = subs.iter().find(|s| s.id == sub.id).unwrap();
    assert_eq!(s.name, "renamed");
    assert_eq!(s.user_agent, None);
    assert_eq!(s.node_count, 0);
}

/// update: non-existent id returns error, does not persist.
#[test]
fn subscription_store_update_returns_error_for_missing_id() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let err = store
        .update(Uuid::new_v4(), "x", "https://example.com", None)
        .unwrap_err();
    assert!(err.to_string().contains("does not exist"));
    assert!(store.load().unwrap().is_empty());
}

// ---------- Subscription content cache (data_dir/subscription_cache/<id>.json) ----------

/// Write-read roundtrip: not written → None; written → read back as-is;
/// file path matches convention; missing field old cache (`{}`) deserializes
/// to defaults.
#[test]
fn subscription_cache_write_read_roundtrip_and_legacy() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("sub", "https://example.com/sub", true, None)
        .unwrap();

    // Not written → None.
    assert!(store.load_cached_content(sub.id).is_none());

    let cached = CachedSubscriptionContent {
        format: SubFormat::ShareLinks,
        singbox_nodes: vec![serde_json::json!({ "tag": "n1", "type": "vless" })],
        mihomo_nodes: vec![serde_json::json!({ "name": "n1", "type": "vless" })],
    };
    store.write_cached_content(sub.id, &cached).unwrap();
    assert_eq!(store.load_cached_content(sub.id), Some(cached));

    // File path matches convention: data_dir/subscription_cache/<id>.json.
    let path = store.cache_file(sub.id);
    assert!(path.starts_with(dir.path()));
    assert!(path.ends_with(format!("{}.json", sub.id)));
    assert!(path.exists());

    // Missing field old cache → `#[serde(default)]` compatible, no error.
    let dir2 = tempfile::tempdir().unwrap();
    let store2 = SubscriptionStore::new(dir2.path().to_path_buf());
    let sub2 = store2
        .add("sub2", "https://example.com/sub2", true, None)
        .unwrap();
    let legacy_path = store2.cache_file(sub2.id);
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::write(&legacy_path, "{}").unwrap();
    let legacy = store2.load_cached_content(sub2.id).unwrap();
    assert_eq!(legacy.format, SubFormat::ShareLinks);
    assert!(legacy.singbox_nodes.is_empty());
    assert!(legacy.mihomo_nodes.is_empty());
}

/// Corrupted cache file → `None` (logs warn, no error, no panic).
#[test]
fn subscription_cache_corrupted_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("sub", "https://example.com/sub", true, None)
        .unwrap();
    let path = store.cache_file(sub.id);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not json").unwrap();
    assert!(store.load_cached_content(sub.id).is_none());
}

/// URL change → cache file deleted; URL unchanged (only name changed) →
/// cache retained.
#[test]
fn subscription_cache_cleared_on_url_change() {
    let dir = tempfile::tempdir().unwrap();
    let store = SubscriptionStore::new(dir.path().to_path_buf());
    let sub = store
        .add("sub", "https://example.com/a", true, None)
        .unwrap();
    store
        .write_cached_content(
            sub.id,
            &CachedSubscriptionContent {
                format: SubFormat::SingBoxJson,
                singbox_nodes: vec![serde_json::json!({ "tag": "n1" })],
                mihomo_nodes: Vec::new(),
            },
        )
        .unwrap();
    assert!(store.load_cached_content(sub.id).is_some());

    // URL change → cache deleted.
    store
        .update(sub.id, "sub", "https://example.com/b", None)
        .unwrap();
    assert!(store.load_cached_content(sub.id).is_none());

    // URL unchanged (only name changed) → cache retained.
    store
        .write_cached_content(
            sub.id,
            &CachedSubscriptionContent {
                format: SubFormat::SingBoxJson,
                singbox_nodes: vec![serde_json::json!({ "tag": "n1" })],
                mihomo_nodes: Vec::new(),
            },
        )
        .unwrap();
    store
        .update(sub.id, "renamed", "https://example.com/b", None)
        .unwrap();
    assert!(store.load_cached_content(sub.id).is_some());
}
