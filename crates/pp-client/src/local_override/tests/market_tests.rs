//! Market source fetch / cache tests (split out of `market.rs` to stay within
//! the business-file size gate; see `.agents/rules/code-organization.md`).

use std::collections::HashSet;

use super::*;

#[test]
fn parse_filters_invalid_entries_and_defaults_optional_fields() {
    let text = r#"[
        {"id":"ads","name":"广告","description":"d","category":"广告","format":"binary","url":"https://e/ads.srs"},
        {"id":"cn","name":"中国","format":"source","url":"https://e/cn.json"},
        {"id":"","name":"no id","format":"binary","url":"https://e/x.srs"},
        {"id":"nourl","name":"no url","format":"binary"},
        {"id":"badformat","name":"bad","format":"yaml","url":"https://e/x"},
        "not-an-object"
    ]"#;
    let entries = parse_market_entries(text).unwrap();
    assert_eq!(entries.len(), 2, "非法条目应被过滤：{entries:?}");
    assert_eq!(entries[0].id, "ads");
    assert_eq!(entries[0].description, "d");
    assert_eq!(entries[0].category, "广告");
    assert_eq!(entries[0].format, RuleSetFormat::Binary);
    // 缺省 description / category → 空串。
    assert_eq!(entries[1].id, "cn");
    assert_eq!(entries[1].description, "");
    assert_eq!(entries[1].category, "");
    assert_eq!(entries[1].format, RuleSetFormat::Source);
}

#[test]
fn parse_rejects_non_array_and_invalid_json() {
    assert!(parse_market_entries("{\"a\":1}").is_err());
    assert!(parse_market_entries("not json").is_err());
    assert!(parse_market_entries("[]").unwrap().is_empty());
}

#[test]
fn cache_write_read_remove_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let manager = MarketManager::new(dir.path().to_path_buf());
    let raw = r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#;
    manager.write_cache("src-1", raw).unwrap();

    let path = manager.market_source_file_path("src-1");
    assert!(path.exists());
    assert_eq!(
        path,
        dir.path()
            .join("rulesets")
            .join("market")
            .join("src-1.json")
    );
    // 原样落盘。
    assert_eq!(std::fs::read_to_string(&path).unwrap(), raw);

    let entries = manager.read_cache_entries("src-1");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "ads");
    assert_eq!(manager.cached_entry_count("src-1"), 1);

    manager.remove_cache("src-1").unwrap();
    assert!(!path.exists());
    // 删除不存在的缓存文件不报错。
    manager.remove_cache("src-1").unwrap();
    assert_eq!(manager.cached_entry_count("src-1"), 0);
}

#[test]
fn cleanup_removes_orphaned_caches_only() {
    let dir = tempfile::tempdir().unwrap();
    let manager = MarketManager::new(dir.path().to_path_buf());
    manager.write_cache("keep", "[]").unwrap();
    manager.write_cache("drop", "[]").unwrap();

    let mut keep = HashSet::new();
    keep.insert("keep".to_string());
    manager.cleanup_caches(&keep);

    assert!(manager.market_source_file_path("keep").exists());
    assert!(!manager.market_source_file_path("drop").exists());
}

/// 拉取链路：本地 axum 服务 200 → 原样返回 JSON 文本 + 解析条目；
/// 非 2xx / 非法 JSON → 报错（调用方据此不落盘）。
#[tokio::test]
async fn fetch_catalog_parses_200_and_errors_on_failure() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let body = r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#;
    let app = axum::Router::new()
        .route(
            "/ok",
            axum::routing::get(move || async move { body.to_string() }),
        )
        .route("/notjson", axum::routing::get(|| async { "not json" }))
        .route(
            "/missing",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let dir = tempfile::tempdir().unwrap();
    let manager = MarketManager::new(dir.path().to_path_buf());

    let (raw, entries) = manager
        .fetch_catalog(&format!("http://{addr}/ok"))
        .await
        .unwrap();
    assert_eq!(raw, body);
    assert_eq!(entries.len(), 1);

    assert!(
        manager
            .fetch_catalog(&format!("http://{addr}/notjson"))
            .await
            .is_err()
    );
    assert!(
        manager
            .fetch_catalog(&format!("http://{addr}/missing"))
            .await
            .is_err()
    );
}
