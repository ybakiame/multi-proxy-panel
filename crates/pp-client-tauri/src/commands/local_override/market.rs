//! Local Override 市场源命令（用户自定义远程目录）。
//!
//! 内置静态市场目录已取消，市场内容全部来自用户添加的远程 JSON 目录源：
//! - `local_override_market_add`：拉取验证 → 落盘缓存 → 追加源 → save；
//! - `local_override_market_remove`：删源 + 清理缓存文件；
//! - `local_override_market_refresh`：重拉 → 落盘 → 更新 last_fetched → save；
//! - `local_override_market_entries`：读取全部源缓存条目（纯读，不拉网络）。
//!
//! 缓存路径：`data_dir/rulesets/market/<source_id>.json`。

use std::collections::HashSet;

use pp_client::local_override::{LocalOverrideStore, MarketEntry, MarketManager, MarketSource};
use tauri::State;

use super::views::{MarketEntryView, MarketSourceView};
use crate::state::AppState;

/// Current Unix time in seconds.
fn now_sec() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// `MarketEntry` → `MarketEntryView`（带来源标注）。
fn entry_view(entry: MarketEntry, source: &MarketSource) -> MarketEntryView {
    MarketEntryView {
        id: entry.id,
        name: entry.name,
        description: entry.description,
        category: entry.category,
        format: entry.format,
        url: entry.url,
        source_id: source.id.clone(),
        source_name: source.name.clone(),
    }
}

/// 添加市场源：名称/URL 必填、URL 唯一；拉取解析失败即报错且不保存。
pub(crate) async fn run_market_add(
    data_dir: &std::path::Path,
    name: String,
    url: String,
) -> Result<MarketSourceView, String> {
    let name = name.trim().to_string();
    let url = url.trim().to_string();
    if name.is_empty() {
        return Err("市场源名称不能为空".to_string());
    }
    if url.is_empty() {
        return Err("市场源 URL 不能为空".to_string());
    }

    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    if ovr.market_sources.iter().any(|s| s.url == url) {
        return Err(format!("市场源 URL 已存在：{url}"));
    }

    let manager = MarketManager::new(data_dir.to_path_buf());
    // 拉取 + 解析验证：失败直接返回，不落盘、不保存。
    let (raw, entries) = manager
        .fetch_catalog(&url)
        .await
        .map_err(|e| format!("拉取市场目录失败：{e}"))?;

    let id = uuid::Uuid::new_v4().to_string();
    manager
        .write_cache(&id, &raw)
        .map_err(|e| format!("写入市场缓存失败：{e}"))?;

    let source = MarketSource {
        id: id.clone(),
        name: name.clone(),
        url: url.clone(),
        last_fetched: now_sec(),
    };
    ovr.market_sources.push(source);
    if let Err(e) = store.save(&ovr) {
        // 保存失败：清理刚写入的孤儿缓存，保持磁盘一致。
        let _ = manager.remove_cache(&id);
        return Err(format!("failed to save local override: {e}"));
    }

    Ok(MarketSourceView {
        id,
        name,
        url,
        last_fetched: ovr.market_sources.last().map_or(0, |s| s.last_fetched),
        entry_count: entries.len(),
    })
}

/// 删除市场源 + 清理其缓存文件；返回是否移除了源。
pub(crate) fn run_market_remove(data_dir: &std::path::Path, id: &str) -> Result<bool, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    let before = ovr.market_sources.len();
    ovr.market_sources.retain(|s| s.id != id);
    if ovr.market_sources.len() == before {
        return Ok(false);
    }
    store
        .save(&ovr)
        .map_err(|e| format!("failed to save local override: {e}"))?;

    let manager = MarketManager::new(data_dir.to_path_buf());
    manager
        .remove_cache(id)
        .map_err(|e| format!("failed to remove market cache: {e}"))?;
    // 兜底清理其他孤儿缓存（一般无，best-effort）。
    let keep: HashSet<String> = ovr.market_sources.iter().map(|s| s.id.clone()).collect();
    manager.cleanup_caches(&keep);
    Ok(true)
}

/// 刷新单个市场源：重拉成功才落盘 + 更新 `last_fetched`；失败保留旧缓存并报错。
pub(crate) async fn run_market_refresh(
    data_dir: &std::path::Path,
    id: &str,
) -> Result<usize, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    let Some(idx) = ovr.market_sources.iter().position(|s| s.id == id) else {
        return Err(format!("市场源不存在：{id}"));
    };
    let url = ovr.market_sources[idx].url.clone();

    let manager = MarketManager::new(data_dir.to_path_buf());
    // 失败返回 Err：旧缓存保持不动。
    let (raw, entries) = manager
        .fetch_catalog(&url)
        .await
        .map_err(|e| format!("拉取市场目录失败：{e}"))?;
    manager
        .write_cache(id, &raw)
        .map_err(|e| format!("写入市场缓存失败：{e}"))?;

    ovr.market_sources[idx].last_fetched = now_sec();
    store
        .save(&ovr)
        .map_err(|e| format!("failed to save local override: {e}"))?;

    Ok(entries.len())
}

/// 读取全部源的缓存条目（合并、带来源标注）；纯读缓存，不拉网络。
pub(crate) fn run_market_entries(
    data_dir: &std::path::Path,
) -> Result<Vec<MarketEntryView>, String> {
    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    let manager = MarketManager::new(data_dir.to_path_buf());
    let mut entries = Vec::new();
    for source in &ovr.market_sources {
        for entry in manager.read_cache_entries(&source.id) {
            entries.push(entry_view(entry, source));
        }
    }
    Ok(entries)
}

/// 添加市场源（拉取验证 + 缓存 + 保存）。
#[tauri::command]
pub async fn local_override_market_add(
    state: State<'_, AppState>,
    name: String,
    url: String,
) -> Result<MarketSourceView, String> {
    run_market_add(&state.data_dir, name, url).await
}

/// 删除市场源并清理缓存文件。
#[tauri::command]
pub fn local_override_market_remove(
    state: State<'_, AppState>,
    id: String,
) -> Result<bool, String> {
    run_market_remove(&state.data_dir, &id)
}

/// 刷新市场源，返回条目数。
#[tauri::command]
pub async fn local_override_market_refresh(
    state: State<'_, AppState>,
    id: String,
) -> Result<usize, String> {
    run_market_refresh(&state.data_dir, &id).await
}

/// 读取全部源的缓存条目（纯读缓存）。
#[tauri::command]
pub fn local_override_market_entries(
    state: State<'_, AppState>,
) -> Result<Vec<MarketEntryView>, String> {
    run_market_entries(&state.data_dir)
}

#[cfg(test)]
mod tests {
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

        let view = run_market_add(dir.path(), "示例市场".to_string(), url.clone())
            .await
            .unwrap();
        assert_eq!(view.name, "示例市场");
        assert_eq!(view.entry_count, 1);
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
        let err = run_market_add(dir.path(), "重复".to_string(), url)
            .await
            .unwrap_err();
        assert!(err.contains("已存在"), "{err}");
    }

    #[tokio::test]
    async fn add_rejects_invalid_catalog_without_saving() {
        let url = spawn_catalog_server("not a json array").await;
        let dir = tempfile::tempdir().unwrap();

        let err = run_market_add(dir.path(), "坏源".to_string(), url)
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
        let added = run_market_add(dir.path(), "市场".to_string(), url)
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
        let added = run_market_add(dir.path(), "市场".to_string(), url.clone())
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
        let added = run_market_add(dir.path(), "市场".to_string(), url)
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
        let added = run_market_add(dir.path(), "示例市场".to_string(), url)
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
                market_sources: vec![MarketSource {
                    id: "m1".to_string(),
                    name: "空源".to_string(),
                    url: "https://e/m.json".to_string(),
                    last_fetched: 0,
                }],
                ..Default::default()
            })
            .unwrap();
        assert!(run_market_entries(dir.path()).unwrap().is_empty());
    }
}
