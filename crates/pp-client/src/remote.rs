//! 远程资源订阅（脚本 / 重写规则 URL 定时拉取）。
//!
//! 管理用户在客户端配置的远程订阅：纯 JS 脚本（[`RemoteKind::Script`]）与
//! QX / Surge / Loon 配置片段（[`RemoteKind::Snippet`]，经 [`parse_import`] 解析）。
//!
//! 拉取结果落盘缓存：
//! - Script → `data_dir/scripts/<name>.js`
//! - Snippet → 解析 + 回填脚本 URL 源码后聚合为 `data_dir/remote_cache/<name>.json`
//!   （重写规则的 `Regex` 以 pattern 字符串持久化，读回时重编译）
//!
//! 运行时通过 [`RemoteManager::load_cached`] 读取全部缓存并合并为
//! [`MergedRemoteConfig`]，供 MITM（rewrite / 脚本钩子 / hostname）与
//! 定时调度器（task 脚本）使用。

use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};
use pp_mitm::{Phase, RewriteKind, RewriteRule, ScriptRule};
use pp_script::{Notifier, ScriptDialect, ScriptKind, TaskScript};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::import::{ImportedConfig, parse_import};

/// 一条远程订阅资源。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteResource {
    /// 资源名（脚本落盘文件名 / 缓存文件名）。
    pub name: String,
    /// 远程 URL（http/https）。
    pub url: String,
    /// 资源类型。
    pub kind: RemoteKind,
    /// Snippet 片段使用的脚本方言。
    pub dialect: ScriptDialect,
    /// 更新间隔（秒），默认 86400（每日）。
    pub update_interval_secs: u64,
    /// 是否启用，默认启用。
    pub enabled: bool,
    /// 资源描述（可选；新增字段，旧清单缺省为 `None`）。
    pub description: Option<String>,
}

impl Default for RemoteResource {
    fn default() -> Self {
        Self {
            name: String::new(),
            url: String::new(),
            kind: RemoteKind::Script,
            dialect: ScriptDialect::Surge,
            update_interval_secs: 86400,
            enabled: true,
            description: None,
        }
    }
}

/// 远程资源类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteKind {
    /// 纯 JS 脚本文件，原文落盘 `scripts/<name>.js`。
    Script,
    /// QX / Surge / Loon 配置片段，经 `parse_import` 解析后聚合缓存。
    Snippet,
}

/// 根据 URL 后缀嗅探远端资源类型与脚本方言。
///
/// - `.sgmodule` → `(Snippet, Surge)`
/// - `.plugin` / `.loon` → `(Snippet, Loon)`
/// - `.conf` → `(Snippet, QuantumultX)`
/// - `.js` → `(Script, QuantumultX)`（QX 风格脚本最常见，默认 QX）
/// - 其他 / 无后缀 → `None`
///
/// 后缀判定忽略 query / fragment（`?token=...` 等不影响后缀）。
pub fn detect_resource_from_url(url: &str) -> Option<(RemoteKind, ScriptDialect)> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("sgmodule") => Some((RemoteKind::Snippet, ScriptDialect::Surge)),
        Some("plugin") | Some("loon") => Some((RemoteKind::Snippet, ScriptDialect::Loon)),
        Some("conf") => Some((RemoteKind::Snippet, ScriptDialect::QuantumultX)),
        Some("js") => Some((RemoteKind::Script, ScriptDialect::QuantumultX)),
        _ => None,
    }
}

/// 一次 `fetch_all` 的拉取报告。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchReport {
    /// 成功拉取并落盘的远程资源数（每个 enabled remote 计一次）。
    pub fetched: usize,
    /// 落盘的纯 JS 脚本数（Script）或回填的脚本钩子数（Snippet）。
    pub scripts: usize,
    /// Snippet 中解析出的重写规则数。
    pub rewrites: usize,
    /// Snippet 中解析出的定时任务数。
    pub tasks: usize,
    /// 拉取/解析/回填失败与偏差的警告列表。
    pub warnings: Vec<String>,
}

/// 一次本地导入合并进缓存的摘要。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// 合并进缓存的重写规则数。
    pub rewrites: usize,
    /// 合并进缓存的脚本钩子数（`source` 非空才会合并）。
    pub scripts: usize,
    /// 合并进缓存的定时任务数（`source` 非空才会合并）。
    pub tasks: usize,
    /// 本次导入贡献的 hostname 数。
    pub hostnames: usize,
    /// 跳过（source 为空未拉取）与解析偏差的警告列表。
    pub warnings: Vec<String>,
}

/// 全部远程订阅缓存合并后的运行时配置。
#[derive(Default)]
pub struct MergedRemoteConfig {
    /// 重写规则（pattern 已重编译）。
    pub rewrites: Vec<RewriteRule>,
    /// 脚本钩子规则（`source` 已回填）。
    pub scripts: Vec<ScriptRule>,
    /// 定时任务（`source` 已回填）。
    pub task_scripts: Vec<TaskScript>,
    /// MITM 主机名白名单（已去重）。
    pub hostnames: Vec<String>,
}

/// 远程资源管理器：负责 remotes 清单读写、定时拉取与缓存读取。
#[derive(Debug, Clone)]
pub struct RemoteManager {
    data_dir: PathBuf,
    client: reqwest::Client,
}

impl RemoteManager {
    /// 基于数据目录创建管理器（30 秒请求超时，禁用系统代理）。
    pub fn new(data_dir: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { data_dir, client }
    }

    /// 远程清单路径：`data_dir/remotes.json`。
    pub fn remotes_file(&self) -> PathBuf {
        self.data_dir.join("remotes.json")
    }

    /// 读取远程资源清单；文件不存在时返回空列表。
    pub fn load(&self) -> PanelResult<Vec<RemoteResource>> {
        let path = self.remotes_file();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    /// 保存远程资源清单到 `data_dir/remotes.json`。
    pub fn save(&self, remotes: &[RemoteResource]) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(remotes)?;
        std::fs::write(self.remotes_file(), text)?;
        Ok(())
    }

    /// 拉取全部启用的远程资源并落盘缓存。
    ///
    /// 单个资源失败仅记入 [`FetchReport::warnings`]，不影响其他资源。
    pub async fn fetch_all(&self, remotes: &[RemoteResource]) -> FetchReport {
        let mut report = FetchReport::default();
        for remote in remotes.iter().filter(|r| r.enabled) {
            match remote.kind {
                RemoteKind::Script => match self.fetch_text(&remote.url).await {
                    Ok(text) => {
                        if let Err(e) = self.write_script(&remote.name, &text) {
                            report.warnings.push(format!(
                                "remote '{}': write script failed: {e}",
                                remote.name
                            ));
                            continue;
                        }
                        report.fetched += 1;
                        report.scripts += 1;
                    }
                    Err(e) => report
                        .warnings
                        .push(format!("remote '{}': {e}", remote.name)),
                },
                RemoteKind::Snippet => match self.fetch_snippet(remote).await {
                    Ok(imported) => {
                        report.rewrites += imported.rewrites.len();
                        report.scripts += imported.scripts.len();
                        report.tasks += imported.task_scripts.len();
                        for w in &imported.warnings {
                            report
                                .warnings
                                .push(format!("remote '{}': {w}", remote.name));
                        }
                        if let Err(e) = self.write_cache(&remote.name, &imported) {
                            report
                                .warnings
                                .push(format!("remote '{}': write cache failed: {e}", remote.name));
                            continue;
                        }
                        report.fetched += 1;
                    }
                    Err(e) => report
                        .warnings
                        .push(format!("remote '{}': {e}", remote.name)),
                },
            }
        }
        report
    }

    /// 读取全部 Snippet 缓存并合并为运行时配置。
    ///
    /// 单个缓存文件损坏/缺失仅记 warning 跳过，不阻塞其他缓存。
    pub fn load_cached(&self) -> PanelResult<MergedRemoteConfig> {
        let cache_dir = self.data_dir.join("remote_cache");
        let mut merged = MergedRemoteConfig::default();
        if !cache_dir.is_dir() {
            return Ok(merged);
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&cache_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x == "json")
            })
            .collect();
        paths.sort();
        for path in paths {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(path = %path.display(), "read remote cache failed: {e}");
                    continue;
                }
            };
            let cached: CachedRemoteConfig = match serde_json::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), "parse remote cache failed: {e}");
                    continue;
                }
            };
            let part = cached.into_merged();
            merged.rewrites.extend(part.rewrites);
            merged.scripts.extend(part.scripts);
            merged.task_scripts.extend(part.task_scripts);
            for hostname in part.hostnames {
                if !merged.hostnames.contains(&hostname) {
                    merged.hostnames.push(hostname);
                }
            }
        }
        Ok(merged)
    }

    /// 将一次本地导入的 [`ImportedConfig`] 合并进固定缓存 `remote_cache/imported.json`。
    ///
    /// 本地导入不做远端拉取（脚本钩子 / 定时任务的 `source` 为空且 URL 未知不可用），
    /// 因此 `source` 为空的脚本钩子与任务会被跳过并计入 [`ImportSummary::warnings`]；
    /// 重写规则与 hostname 直接合入。重复导入在既有缓存上追加（不覆盖）。
    pub fn merge_imported(&self, imported: &ImportedConfig) -> PanelResult<ImportSummary> {
        let mut summary = ImportSummary::default();
        summary.warnings.extend(imported.warnings.iter().cloned());

        let cache_dir = self.data_dir.join("remote_cache");
        let cache_path = cache_dir.join("imported.json");
        let mut cached = CachedRemoteConfig::default();
        if cache_path.exists() {
            match std::fs::read_to_string(&cache_path) {
                Ok(text) => match serde_json::from_str::<CachedRemoteConfig>(&text) {
                    Ok(c) => cached = c,
                    Err(e) => summary.warnings.push(format!(
                        "existing local import cache unreadable, start fresh: {e}"
                    )),
                },
                Err(e) => summary.warnings.push(format!(
                    "existing local import cache unreadable, start fresh: {e}"
                )),
            }
        }

        for (rule, (name, url)) in imported.scripts.iter().zip(imported.script_urls.iter()) {
            if rule.source.is_empty() {
                summary.warnings.push(format!(
                    "script '{name}' source not fetched ({url}), skipped"
                ));
                continue;
            }
            cached.scripts.push(CachedScriptRule::from(rule));
            summary.scripts += 1;
        }
        for (task, url) in &imported.task_scripts {
            if task.source.is_empty() {
                summary.warnings.push(format!(
                    "task '{}' source not fetched ({url}), skipped",
                    task.name
                ));
                continue;
            }
            cached.task_scripts.push(task.clone());
            summary.tasks += 1;
        }
        cached
            .rewrites
            .extend(imported.rewrites.iter().map(CachedRewriteRule::from));
        summary.rewrites = imported.rewrites.len();
        for hostname in &imported.hostnames {
            if !cached.hostnames.contains(hostname) {
                cached.hostnames.push(hostname.clone());
            }
        }
        summary.hostnames = imported.hostnames.len();

        std::fs::create_dir_all(&cache_dir)?;
        let text = serde_json::to_string_pretty(&cached)?;
        std::fs::write(&cache_path, text)?;
        Ok(summary)
    }

    /// 拉取单个 URL 的文本内容；非 2xx 视为失败。
    async fn fetch_text(&self, url: &str) -> PanelResult<String> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| PanelError::Client(format!("remote fetch failed ({url}): {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(PanelError::Client(format!(
                "remote fetch returned HTTP {status} ({url})"
            )));
        }
        resp.text()
            .await
            .map_err(|e| PanelError::Client(format!("failed to read remote body ({url}): {e}")))
    }

    /// 拉取 Snippet 片段：解析 → 逐个回填脚本钩子/任务源码（失败记 warning 跳过）。
    async fn fetch_snippet(&self, remote: &RemoteResource) -> PanelResult<ImportedConfig> {
        let text = self.fetch_text(&remote.url).await?;
        let mut imported = parse_import(&text, remote.dialect).map_err(|e| {
            PanelError::Client(format!("snippet '{}' parse failed: {e}", remote.name))
        })?;

        let scripts = std::mem::take(&mut imported.scripts);
        let script_urls = std::mem::take(&mut imported.script_urls);
        let mut kept_scripts = Vec::new();
        for (rule, (_name, url)) in scripts.into_iter().zip(script_urls) {
            match self.fetch_text(&url).await {
                Ok(source) => {
                    let mut rule = rule;
                    rule.source = source;
                    kept_scripts.push(rule);
                }
                Err(e) => imported
                    .warnings
                    .push(format!("hook script fetch failed: {e}")),
            }
        }
        imported.scripts = kept_scripts;

        let task_scripts = std::mem::take(&mut imported.task_scripts);
        let mut kept_tasks = Vec::new();
        for (mut task, url) in task_scripts {
            match self.fetch_text(&url).await {
                Ok(source) => {
                    task.source = source;
                    kept_tasks.push((task, url));
                }
                Err(e) => imported
                    .warnings
                    .push(format!("task '{}' script fetch failed: {e}", task.name)),
            }
        }
        imported.task_scripts = kept_tasks;
        Ok(imported)
    }

    /// 将纯 JS 脚本原文落盘 `data_dir/scripts/<name>.js`。
    fn write_script(&self, name: &str, content: &str) -> PanelResult<PathBuf> {
        let dir = self.data_dir.join("scripts");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.js", safe_name(name)));
        std::fs::write(&path, content)?;
        Ok(path)
    }

    /// 将 Snippet 聚合结果序列化为缓存 `data_dir/remote_cache/<name>.json`。
    fn write_cache(&self, name: &str, imported: &ImportedConfig) -> PanelResult<PathBuf> {
        let dir = self.data_dir.join("remote_cache");
        std::fs::create_dir_all(&dir)?;
        let cached = CachedRemoteConfig::from_imported(imported);
        let text = serde_json::to_string_pretty(&cached)?;
        let path = dir.join(format!("{}.json", safe_name(name)));
        std::fs::write(&path, text)?;
        Ok(path)
    }
}

/// 文件名安全化：路径分隔符替换为 `_`，避免资源名造成目录穿越。
fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect()
}

/// 使用 tracing 记录桌面通知的 Notifier（OS 原生通知后续接入 Tauri）。
#[derive(Debug, Default)]
pub struct TracingNotifier;

impl TracingNotifier {
    /// 创建通知器。
    pub fn new() -> Self {
        Self
    }
}

impl Notifier for TracingNotifier {
    fn notify(&self, title: &str, subtitle: &str, body: &str, options: Option<serde_json::Value>) {
        tracing::info!(title, subtitle, body, options = ?options, "desktop notification");
    }
}

/// 可落盘的 Snippet 缓存视图：`Regex` 以 pattern 字符串持久化。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CachedRemoteConfig {
    rewrites: Vec<CachedRewriteRule>,
    scripts: Vec<CachedScriptRule>,
    task_scripts: Vec<TaskScript>,
    hostnames: Vec<String>,
}

impl CachedRemoteConfig {
    fn from_imported(cfg: &ImportedConfig) -> Self {
        Self {
            rewrites: cfg.rewrites.iter().map(CachedRewriteRule::from).collect(),
            scripts: cfg.scripts.iter().map(CachedScriptRule::from).collect(),
            task_scripts: cfg
                .task_scripts
                .iter()
                .map(|(task, _)| task.clone())
                .collect(),
            hostnames: cfg.hostnames.clone(),
        }
    }

    fn into_merged(self) -> MergedRemoteConfig {
        let mut merged = MergedRemoteConfig::default();
        for rule in self.rewrites {
            match rule.into_rule() {
                Some(rule) => merged.rewrites.push(rule),
                None => tracing::warn!("cached rewrite pattern invalid, skipped"),
            }
        }
        for rule in self.scripts {
            match rule.into_rule() {
                Some(rule) => merged.scripts.push(rule),
                None => tracing::warn!("cached script pattern invalid, skipped"),
            }
        }
        merged.task_scripts = self.task_scripts;
        merged.hostnames = self.hostnames;
        merged
    }
}

/// 缓存用重写规则（pattern 字符串化）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedRewriteRule {
    kind: CachedRewriteKind,
    pattern: String,
}

impl From<&RewriteRule> for CachedRewriteRule {
    fn from(rule: &RewriteRule) -> Self {
        Self {
            kind: CachedRewriteKind::from(&rule.kind),
            pattern: rule.pattern.as_str().to_string(),
        }
    }
}

impl CachedRewriteRule {
    fn into_rule(self) -> Option<RewriteRule> {
        let pattern = Regex::new(&self.pattern).ok()?;
        Some(RewriteRule {
            kind: self.kind.into_kind()?,
            pattern,
        })
    }
}

/// 缓存用重写动作（tagged enum，避免序列化 `Phase` 依赖 pp-mitm 内部类型）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CachedRewriteKind {
    UrlRewrite {
        target: String,
    },
    HeaderRewrite {
        phase: CachedPhase,
        name: String,
        value: Option<String>,
    },
    BodyRewrite {
        phase: CachedPhase,
        replacement: String,
    },
    Reject,
    Mock {
        status: u16,
        body: String,
    },
}

impl From<&RewriteKind> for CachedRewriteKind {
    fn from(kind: &RewriteKind) -> Self {
        match kind {
            RewriteKind::UrlRewrite { target } => CachedRewriteKind::UrlRewrite {
                target: target.clone(),
            },
            RewriteKind::HeaderRewrite { phase, name, value } => CachedRewriteKind::HeaderRewrite {
                phase: (*phase).into(),
                name: name.clone(),
                value: value.clone(),
            },
            RewriteKind::BodyRewrite { phase, replacement } => CachedRewriteKind::BodyRewrite {
                phase: (*phase).into(),
                replacement: replacement.clone(),
            },
            RewriteKind::Reject => CachedRewriteKind::Reject,
            RewriteKind::Mock { status, body } => CachedRewriteKind::Mock {
                status: *status,
                body: body.clone(),
            },
        }
    }
}

impl CachedRewriteKind {
    fn into_kind(self) -> Option<RewriteKind> {
        Some(match self {
            CachedRewriteKind::UrlRewrite { target } => RewriteKind::UrlRewrite { target },
            CachedRewriteKind::HeaderRewrite { phase, name, value } => RewriteKind::HeaderRewrite {
                phase: phase.into(),
                name,
                value,
            },
            CachedRewriteKind::BodyRewrite { phase, replacement } => RewriteKind::BodyRewrite {
                phase: phase.into(),
                replacement,
            },
            CachedRewriteKind::Reject => RewriteKind::Reject,
            CachedRewriteKind::Mock { status, body } => RewriteKind::Mock { status, body },
        })
    }
}

/// 缓存用代理阶段。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CachedPhase {
    Request,
    Response,
}

impl From<Phase> for CachedPhase {
    fn from(p: Phase) -> Self {
        match p {
            Phase::Request => CachedPhase::Request,
            Phase::Response => CachedPhase::Response,
        }
    }
}

impl From<CachedPhase> for Phase {
    fn from(p: CachedPhase) -> Self {
        match p {
            CachedPhase::Request => Phase::Request,
            CachedPhase::Response => Phase::Response,
        }
    }
}

/// 缓存用脚本钩子规则（pattern 字符串化，`source` 已回填）。
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedScriptRule {
    name: String,
    kind: ScriptKind,
    pattern: String,
    requires_body: bool,
    max_size: usize,
    source: String,
}

impl From<&ScriptRule> for CachedScriptRule {
    fn from(rule: &ScriptRule) -> Self {
        Self {
            name: rule.name.clone(),
            kind: rule.kind,
            pattern: rule.pattern.as_str().to_string(),
            requires_body: rule.requires_body,
            max_size: rule.max_size,
            source: rule.source.clone(),
        }
    }
}

impl CachedScriptRule {
    fn into_rule(self) -> Option<ScriptRule> {
        let pattern = Regex::new(&self.pattern).ok()?;
        Some(ScriptRule {
            name: self.name,
            kind: self.kind,
            pattern,
            requires_body: self.requires_body,
            max_size: self.max_size,
            source: self.source,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;

    /// 启动本地 HTTP 服务（禁外部网络）：
    /// - `/snippet`：由 `snippet` 闭包基于服务地址生成片段内容
    /// - `/hook.js` / `/task.js` / `/script.js`：固定脚本内容
    /// - `/missing.js`：404
    async fn spawn_remote_server(snippet: impl Fn(&str) -> String) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let snippet = snippet(&base);
        let app = axum::Router::new()
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
                axum::routing::get(|| async { "const task = 2;" }),
            )
            .route(
                "/script.js",
                axum::routing::get(|| async { "const script = 3;" }),
            )
            .route(
                "/missing.js",
                axum::routing::get(|| async { (StatusCode::NOT_FOUND, "not found") }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        base
    }

    #[test]
    fn save_load_roundtrip_applies_defaults_and_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());

        // 文件不存在 → 空列表
        assert!(manager.load().unwrap().is_empty());

        let remotes = vec![RemoteResource {
            name: "rules".into(),
            url: "http://example.com/rules.conf".into(),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::QuantumultX,
            ..RemoteResource::default()
        }];
        manager.save(&remotes).unwrap();
        assert!(manager.remotes_file().exists());
        assert_eq!(manager.load().unwrap(), remotes);
    }

    #[tokio::test]
    async fn fetch_script_downloads_js_to_scripts_dir() {
        let base = spawn_remote_server(|_| String::new()).await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "my-script".into(),
            url: format!("{base}/script.js"),
            kind: RemoteKind::Script,
            ..RemoteResource::default()
        }];

        let report = manager.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1);
        assert_eq!(report.scripts, 1);
        assert!(
            report.warnings.is_empty(),
            "warnings: {:?}",
            report.warnings
        );

        let path = dir.path().join("scripts/my-script.js");
        assert_eq!(std::fs::read_to_string(path).unwrap(), "const script = 3;");
    }

    #[tokio::test]
    async fn fetch_snippet_aggregates_and_recompiles_cached_rules() {
        let base = spawn_remote_server(|base| {
            format!(
                "[rewrite_local]\n\
                 ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
                 ^https?://example\\.com/rsp script-response-body {base}/hook.js\n\
                 \n\
                 [task_local]\n\
                 0 9 * * * {base}/task.js, tag=每日签到\n\
                 \n\
                 [mitm]\n\
                 hostname = *.example.com, api.example2.com\n"
            )
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "rules".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::QuantumultX,
            ..RemoteResource::default()
        }];

        let report = manager.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1);
        assert_eq!(report.rewrites, 1);
        assert_eq!(report.scripts, 1);
        assert_eq!(report.tasks, 1);
        assert!(
            report.warnings.is_empty(),
            "warnings: {:?}",
            report.warnings
        );

        // cache 文件已生成
        assert!(dir.path().join("remote_cache/rules.json").exists());

        // load_cached：重编译 Regex、回填 source、去重 hostname
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert_eq!(
            merged.rewrites[0].pattern.as_str(),
            r"^https?://example\.com/api/(.*)"
        );
        match &merged.rewrites[0].kind {
            RewriteKind::UrlRewrite { target } => {
                assert_eq!(target, "https://cdn.example.com/api/$1");
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].name, "hook-0");
        assert_eq!(merged.scripts[0].source, "const hook = 1;");
        assert_eq!(merged.task_scripts.len(), 1);
        assert_eq!(merged.task_scripts[0].name, "每日签到");
        assert_eq!(merged.task_scripts[0].source, "const task = 2;");
        assert_eq!(
            merged.hostnames,
            vec!["*.example.com".to_string(), "api.example2.com".to_string()]
        );
    }

    #[tokio::test]
    async fn partial_url_failure_records_warning_without_blocking_others() {
        let base = spawn_remote_server(|base| {
            format!(
                "[rewrite_local]\n\
                 ^https?://example\\.com/api/(.*) url-and-header https://cdn.example.com/api/$1\n\
                 ^https?://example\\.com/rsp script-response-body {base}/missing.js\n"
            )
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![
            RemoteResource {
                name: "bad".into(),
                url: format!("{base}/snippet"),
                kind: RemoteKind::Snippet,
                dialect: ScriptDialect::QuantumultX,
                ..RemoteResource::default()
            },
            RemoteResource {
                name: "good".into(),
                url: format!("{base}/script.js"),
                kind: RemoteKind::Script,
                ..RemoteResource::default()
            },
        ];

        let report = manager.fetch_all(&remotes).await;
        // 两个 remote 均拉取成功；bad 的 hook 脚本 404 被跳过
        assert_eq!(report.fetched, 2);
        assert_eq!(report.scripts, 1); // 仅 good 脚本
        assert_eq!(report.rewrites, 1); // bad 的 rewrite 仍缓存
        assert!(
            report.warnings.iter().any(|w| w.contains("missing.js")),
            "warnings: {:?}",
            report.warnings
        );

        // good 落盘、bad snippet 缓存仍生成（rewrite 保留、scripts 跳过）
        assert!(dir.path().join("scripts/good.js").exists());
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert!(merged.scripts.is_empty());
        assert!(merged.task_scripts.is_empty());
    }

    #[test]
    fn merge_imported_keeps_rewrites_hostnames_and_skips_source_empty_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let imported = ImportedConfig {
            rewrites: vec![RewriteRule {
                pattern: Regex::new(r"^https?://example\.com/").unwrap(),
                kind: RewriteKind::Reject,
            }],
            scripts: vec![ScriptRule {
                name: "hook-0".into(),
                kind: ScriptKind::HttpResponse,
                pattern: Regex::new(r"^https?://example\.com/rsp").unwrap(),
                requires_body: true,
                max_size: 131072,
                source: String::new(),
            }],
            script_urls: vec![(
                "hook-0".to_string(),
                "https://example.com/hook.js".to_string(),
            )],
            task_scripts: vec![(
                TaskScript {
                    name: "签到".into(),
                    cron_expr: "0 0 9 * * *".into(),
                    source: String::new(),
                    dialect: ScriptDialect::QuantumultX,
                    enabled: true,
                },
                "https://example.com/task.js".to_string(),
            )],
            hostnames: vec!["*.example.com".to_string()],
            warnings: vec!["parse deviation".to_string()],
            ..Default::default()
        };

        let summary = manager.merge_imported(&imported).unwrap();
        // 重写/hostname 合入；source 为空的脚本与任务跳过计 warning
        assert_eq!(summary.rewrites, 1);
        assert_eq!(summary.hostnames, 1);
        assert_eq!(summary.scripts, 0);
        assert_eq!(summary.tasks, 0);
        assert!(summary.warnings.iter().any(|w| w.contains("hook-0")));
        assert!(summary.warnings.iter().any(|w| w.contains("签到")));
        assert!(
            summary
                .warnings
                .iter()
                .any(|w| w.contains("parse deviation"))
        );

        // 缓存已写入 remote_cache/imported.json 并被 load_cached 读取
        assert!(dir.path().join("remote_cache/imported.json").exists());
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert!(merged.scripts.is_empty());
        assert!(merged.task_scripts.is_empty());
        assert_eq!(merged.hostnames, vec!["*.example.com".to_string()]);
    }

    #[test]
    fn merge_imported_appends_to_existing_import_cache() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());

        let first = ImportedConfig {
            rewrites: vec![RewriteRule {
                pattern: Regex::new(r"^https?://a\.com/").unwrap(),
                kind: RewriteKind::Reject,
            }],
            scripts: vec![],
            script_urls: vec![],
            task_scripts: vec![],
            hostnames: vec!["a.example.com".to_string()],
            warnings: vec![],
            ..Default::default()
        };
        manager.merge_imported(&first).unwrap();

        let second = ImportedConfig {
            rewrites: vec![RewriteRule {
                pattern: Regex::new(r"^https?://b\.com/").unwrap(),
                kind: RewriteKind::UrlRewrite {
                    target: "https://c.com/$1".into(),
                },
            }],
            scripts: vec![],
            script_urls: vec![],
            task_scripts: vec![],
            hostnames: vec!["a.example.com".to_string(), "b.example.com".to_string()],
            warnings: vec![],
            ..Default::default()
        };
        let summary = manager.merge_imported(&second).unwrap();
        assert_eq!(summary.rewrites, 1);
        // 重复 hostname 已在缓存中，不影响本次计数（本次贡献仍为 2）
        assert_eq!(summary.hostnames, 2);

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 2);
        assert_eq!(
            merged.hostnames,
            vec!["a.example.com".to_string(), "b.example.com".to_string()]
        );
    }
}
