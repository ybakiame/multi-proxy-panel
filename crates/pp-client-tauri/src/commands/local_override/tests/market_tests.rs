//! Local Override market command tests (split out of `market.rs` to stay
//! within the business-file size gate; see `.agents/rules/code-organization.md`).

use super::*;
use pp_client::local_override::LocalOverride;

async fn spawn_catalog_server(body: &'static str) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let app = axum::Router::new()
        .route(
            "/market.json",
            axum::routing::get(move || async move { body }),
        )
        .route(
            "/missing",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}/market.json")
}

#[tokio::test]
async fn add_fetches_caches_and_persists_source() {
    let url = spawn_catalog_server(
        r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();

    let view = run_market_add(dir.path(), "示例市场".to_string(), url.clone(), None)
        .await
        .unwrap();
    assert_eq!(view.name, "示例市场");
    assert_eq!(view.entry_count, 1);
    assert_eq!(view.kind, "json");
    assert!(view.last_fetched > 0);

    // 缓存落盘 + 源持久化。
    let manager = MarketManager::new(dir.path().to_path_buf());
    assert!(manager.market_source_file_path(&view.id).exists());
    let reloaded = LocalOverrideStore::new(dir.path().to_path_buf())
        .load()
        .unwrap();
    assert_eq!(reloaded.market_sources.len(), 1);
    assert_eq!(reloaded.market_sources[0].url, url);

    // URL 唯一性校验。
    let err = run_market_add(dir.path(), "重复".to_string(), url, None)
        .await
        .unwrap_err();
    assert!(err.contains("已存在"), "{err}");
}

#[tokio::test]
async fn add_rejects_invalid_catalog_without_saving() {
    let url = spawn_catalog_server("not a json array").await;
    let dir = tempfile::tempdir().unwrap();

    let err = run_market_add(dir.path(), "坏源".to_string(), url, None)
        .await
        .unwrap_err();
    assert!(err.contains("拉取市场目录失败"), "{err}");
    assert!(
        LocalOverrideStore::new(dir.path().to_path_buf())
            .load()
            .unwrap()
            .market_sources
            .is_empty(),
        "验证失败不得保存源"
    );
    let manager = MarketManager::new(dir.path().to_path_buf());
    assert!(
        !manager.market_dir().exists()
            || std::fs::read_dir(manager.market_dir())
                .map(|mut d| d.next().is_none())
                .unwrap_or(true),
        "验证失败不得留下缓存文件"
    );
}

#[tokio::test]
async fn refresh_updates_cache_and_last_fetched() {
    let url = spawn_catalog_server(
        r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let added = run_market_add(dir.path(), "市场".to_string(), url, None)
        .await
        .unwrap();

    // 手工把 last_fetched 归零并覆盖缓存，验证 refresh 会重拉更新。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let mut ovr = store.load().unwrap();
    ovr.market_sources[0].last_fetched = 0;
    store.save(&ovr).unwrap();
    let manager = MarketManager::new(dir.path().to_path_buf());
    manager.write_cache(&added.id, "[]").unwrap();

    let count = run_market_refresh(dir.path(), &added.id).await.unwrap();
    assert_eq!(count, 1);
    let reloaded = store.load().unwrap();
    assert!(reloaded.market_sources[0].last_fetched > 0);
    assert_eq!(manager.cached_entry_count(&added.id), 1);
}

#[tokio::test]
async fn refresh_keeps_old_cache_on_failure() {
    let url = spawn_catalog_server("[]").await;
    let dir = tempfile::tempdir().unwrap();
    let added = run_market_add(dir.path(), "市场".to_string(), url.clone(), None)
        .await
        .unwrap();
    let manager = MarketManager::new(dir.path().to_path_buf());
    let old = std::fs::read_to_string(manager.market_source_file_path(&added.id)).unwrap();

    // 让刷新失败：把 override 中该源的 url 指向返回 404 的端点。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    let mut ovr = store.load().unwrap();
    ovr.market_sources[0].url = url.replace("/market.json", "/missing");
    store.save(&ovr).unwrap();

    assert!(run_market_refresh(dir.path(), &added.id).await.is_err());
    assert_eq!(
        std::fs::read_to_string(manager.market_source_file_path(&added.id)).unwrap(),
        old,
        "刷新失败必须保留旧缓存"
    );
}

#[tokio::test]
async fn remove_deletes_source_and_cache() {
    let url = spawn_catalog_server(
        r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let added = run_market_add(dir.path(), "市场".to_string(), url, None)
        .await
        .unwrap();
    let manager = MarketManager::new(dir.path().to_path_buf());
    assert!(manager.market_source_file_path(&added.id).exists());

    assert!(run_market_remove(dir.path(), &added.id).unwrap());
    assert!(!manager.market_source_file_path(&added.id).exists());
    assert!(
        LocalOverrideStore::new(dir.path().to_path_buf())
            .load()
            .unwrap()
            .market_sources
            .is_empty()
    );
    // 重复删除返回 false。
    assert!(!run_market_remove(dir.path(), &added.id).unwrap());
}

#[tokio::test]
async fn entries_merges_all_source_caches_with_origin() {
    let url = spawn_catalog_server(
        r#"[{"id":"ads","name":"广告","format":"binary","url":"https://e/ads.srs"}]"#,
    )
    .await;
    let dir = tempfile::tempdir().unwrap();
    let added = run_market_add(dir.path(), "示例市场".to_string(), url, None)
        .await
        .unwrap();

    let entries = run_market_entries(dir.path()).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "ads");
    assert_eq!(entries[0].source_id, added.id);
    assert_eq!(entries[0].source_name, "示例市场");
}

#[test]
fn entries_empty_without_sources() {
    let dir = tempfile::tempdir().unwrap();
    // 无源 → 空列表（不报错）。
    assert!(run_market_entries(dir.path()).unwrap().is_empty());
    // 源存在但缓存缺失 → 仍为空。
    let store = LocalOverrideStore::new(dir.path().to_path_buf());
    store
        .save(&LocalOverride {
            market_sources: vec![MarketSource::json(
                "m1".to_string(),
                "空源".to_string(),
                "https://e/m.json".to_string(),
            )],
            ..Default::default()
        })
        .unwrap();
    assert!(run_market_entries(dir.path()).unwrap().is_empty());
}

#[test]
fn detect_github_from_shorthand_and_full_url() {
    // owner/repo 简写（无 tag → latest）。
    assert_eq!(
        detect_market_source("DustinWin/ruleset_geodata", None),
        DetectedMarketSource::Github {
            owner_repo: "DustinWin/ruleset_geodata".to_string(),
            tag: String::new(),
        }
    );
    // 完整 URL（无 tag）。
    assert_eq!(
        detect_market_source("https://github.com/DustinWin/ruleset_geodata", None),
        DetectedMarketSource::Github {
            owner_repo: "DustinWin/ruleset_geodata".to_string(),
            tag: String::new(),
        }
    );
    // URL 内 /releases/tag/<tag> 提取 tag。
    assert_eq!(
        detect_market_source(
            "https://github.com/DustinWin/ruleset_geodata/releases/tag/sing-box-ruleset",
            None
        ),
        DetectedMarketSource::Github {
            owner_repo: "DustinWin/ruleset_geodata".to_string(),
            tag: "sing-box-ruleset".to_string(),
        }
    );
    // 显式 tag 优先于 URL tag。
    assert_eq!(
        detect_market_source(
            "https://github.com/o/r/releases/tag/url-tag",
            Some("explicit-tag")
        ),
        DetectedMarketSource::Github {
            owner_repo: "o/r".to_string(),
            tag: "explicit-tag".to_string(),
        }
    );
    // `.git` 后缀被去除；无 scheme 的 github.com/... 也识别。
    assert_eq!(
        detect_market_source("github.com/o/r.git", None),
        DetectedMarketSource::Github {
            owner_repo: "o/r".to_string(),
            tag: String::new(),
        }
    );
}

#[test]
fn detect_json_for_non_github_inputs() {
    // 完整 JSON URL。
    assert_eq!(
        detect_market_source("https://example.com/market.json", None),
        DetectedMarketSource::Json {
            url: "https://example.com/market.json".to_string(),
        }
    );
    // 无 scheme 但 owner 含点（避免误判为 GitHub 简写）。
    assert_eq!(
        detect_market_source("example.com/market.json", None),
        DetectedMarketSource::Json {
            url: "example.com/market.json".to_string(),
        }
    );
    // 其他域名。
    assert_eq!(
        detect_market_source("https://gitlab.com/o/r", None),
        DetectedMarketSource::Json {
            url: "https://gitlab.com/o/r".to_string(),
        }
    );
}

#[test]
fn github_empty_assets_rejected_json_allowed() {
    let gh = MarketSource::github(
        "id".to_string(),
        "gh".to_string(),
        "o/r".to_string(),
        String::new(),
    );
    let err = validate_market_entries(&gh, &[]).unwrap_err();
    assert!(err.contains("无可用规则集文件"), "{err}");
    // 有资产则通过。
    assert!(
        validate_market_entries(
            &gh,
            &[MarketEntry {
                id: "ads".to_string(),
                name: "ads".to_string(),
                description: String::new(),
                category: String::new(),
                format: pp_client::local_override::RuleSetFormat::Binary,
                url: "https://e/ads.srs".to_string(),
                updated_at: 0,
            }]
        )
        .is_ok()
    );
    // JSON 源空列表不报错（维持现有验证）。
    let json = MarketSource::json(
        "id".to_string(),
        "json".to_string(),
        "https://e/m.json".to_string(),
    );
    assert!(validate_market_entries(&json, &[]).is_ok());
}
