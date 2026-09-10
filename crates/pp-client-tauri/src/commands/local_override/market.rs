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

use pp_client::local_override::{
    LocalOverrideStore, MarketEntry, MarketManager, MarketSource, MarketSourceKind,
};
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
        updated_at: entry.updated_at,
        source_id: source.id.clone(),
        source_name: source.name.clone(),
    }
}

// ---------------------------------------------------------------------------
// Source kind auto-detection
// ---------------------------------------------------------------------------

/// Auto-detected market source from user input.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DetectedMarketSource {
    /// GitHub repository releases (`owner/repo` + optional tag).
    Github { owner_repo: String, tag: String },
    /// Remote JSON catalog URL (existing behavior).
    Json { url: String },
}

/// Detect the market source kind from a single user input.
///
/// 识别规则（`tag` 为显式参数，可为 `None`）：
/// - `github.com/<owner>/<repo>[/releases/tag/<tag>]`（可带/不带 scheme，`www.` 可省）
///   → GitHub；tag 优先取显式参数，其次取 URL 中的 `/releases/tag/<tag>`，空 = latest。
/// - `owner/repo` 简写（owner 仅字母数字与连字符，避免把 `example.com/x.json` 误判）
///   → GitHub，tag 取显式参数，空 = latest。
/// - 其余 → JSON 目录 URL。
fn detect_market_source(input: &str, tag: Option<&str>) -> DetectedMarketSource {
    let raw = input.trim();
    let explicit_tag = tag
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string);

    let without_scheme = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .unwrap_or(raw);
    let host_and_path = without_scheme
        .strip_prefix("www.")
        .unwrap_or(without_scheme);

    if let Some(rest) = host_and_path.strip_prefix("github.com/")
        && let Some((owner, repo, url_tag)) = parse_github_path(rest)
    {
        return DetectedMarketSource::Github {
            owner_repo: format!("{owner}/{repo}"),
            tag: explicit_tag.or(url_tag).unwrap_or_default(),
        };
    }

    if !raw.contains("://")
        && raw.matches('/').count() == 1
        && let Some((owner, repo)) = raw.split_once('/')
        && is_github_owner(owner)
        && is_github_repo(repo)
    {
        return DetectedMarketSource::Github {
            owner_repo: format!("{owner}/{repo}"),
            tag: explicit_tag.unwrap_or_default(),
        };
    }

    DetectedMarketSource::Json {
        url: raw.to_string(),
    }
}

/// Parse `owner/repo[/releases/tag/<tag>]`; returns `(owner, repo, url_tag)`.
fn parse_github_path(rest: &str) -> Option<(String, String, Option<String>)> {
    let mut segs = rest.split('/').filter(|s| !s.is_empty());
    let owner = segs.next()?.to_string();
    let repo = segs.next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    let tail: Vec<&str> = segs.collect();
    let tag = if tail.len() >= 3 && tail[0] == "releases" && tail[1] == "tag" {
        Some(tail[2].to_string())
    } else {
        None
    };
    Some((owner, repo, tag))
}

/// GitHub owner names are alphanumeric + hyphen only (no dots/underscores).
fn is_github_owner(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Repo segment must be non-empty and free of separators / query markers.
fn is_github_repo(s: &str) -> bool {
    !s.is_empty() && !s.contains(['/', ':', '?', '#']) && !s.chars().any(char::is_whitespace)
}

/// 添加时验证：GitHub 源要求过滤后有可用规则集资产；JSON 源维持现有验证（不强制非空）。
fn validate_market_entries(source: &MarketSource, entries: &[MarketEntry]) -> Result<(), String> {
    if matches!(source.kind, MarketSourceKind::GithubReleases { .. }) && entries.is_empty() {
        return Err("该仓库 release 无可用规则集文件".to_string());
    }
    Ok(())
}

/// 添加市场源：名称/URL 必填、解析后 URL 唯一；拉取解析失败即报错且不保存。
/// GitHub 源额外要求 release 过滤后有可用规则集资产（空则报错不保存）。
pub(crate) async fn run_market_add(
    data_dir: &std::path::Path,
    name: String,
    url: String,
    tag: Option<String>,
) -> Result<MarketSourceView, String> {
    let name = name.trim().to_string();
    let input = url.trim().to_string();
    if name.is_empty() {
        return Err("市场源名称不能为空".to_string());
    }
    if input.is_empty() {
        return Err("市场源 URL 不能为空".to_string());
    }

    let id = uuid::Uuid::new_v4().to_string();
    let mut source = match detect_market_source(&input, tag.as_deref()) {
        DetectedMarketSource::Github { owner_repo, tag } => {
            MarketSource::github(id.clone(), name.clone(), owner_repo, tag)
        }
        DetectedMarketSource::Json { url } => MarketSource::json(id.clone(), name.clone(), url),
    };

    let store = LocalOverrideStore::new(data_dir.to_path_buf());
    let mut ovr = store
        .load()
        .map_err(|e| format!("failed to load local override: {e}"))?;
    if ovr.market_sources.iter().any(|s| s.url == source.url) {
        return Err(format!("市场源已存在：{}", source.url));
    }

    let manager = MarketManager::new(data_dir.to_path_buf());
    // 拉取 + 解析验证：失败直接返回，不落盘、不保存。
    let (raw, entries) = manager
        .fetch_for_source(&source)
        .await
        .map_err(|e| format!("拉取市场目录失败：{e}"))?;
    validate_market_entries(&source, &entries)?;

    manager
        .write_cache(&id, &raw)
        .map_err(|e| format!("写入市场缓存失败：{e}"))?;

    let kind = source.kind_str().to_string();
    let resolved_url = source.url.clone();
    source.last_fetched = now_sec();
    ovr.market_sources.push(source);
    if let Err(e) = store.save(&ovr) {
        // 保存失败：清理刚写入的孤儿缓存，保持磁盘一致。
        let _ = manager.remove_cache(&id);
        return Err(format!("failed to save local override: {e}"));
    }

    Ok(MarketSourceView {
        id,
        name,
        url: resolved_url,
        kind,
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
    let source = ovr.market_sources[idx].clone();

    let manager = MarketManager::new(data_dir.to_path_buf());
    // 失败返回 Err：旧缓存保持不动。
    let (raw, entries) = manager
        .fetch_for_source(&source)
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
///
/// `url` 为单输入框内容（`owner/repo` 简写、完整 GitHub URL 或 JSON 目录 URL），
/// 后端自动识别源类型；`tag` 为 GitHub 源可选 release tag（空/缺省 = latest）。
#[tauri::command]
pub async fn local_override_market_add(
    state: State<'_, AppState>,
    name: String,
    url: String,
    tag: Option<String>,
) -> Result<MarketSourceView, String> {
    run_market_add(&state.data_dir, name, url, tag).await
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
#[path = "tests/market_tests.rs"]
mod tests;
