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

use std::collections::HashMap;
use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};
use pp_mitm::{Phase, RewriteKind, RewriteRule, ScriptRule};
use pp_script::{Notifier, ScriptDialect, ScriptKind, TaskScript};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::import::{ConfigMeta, ImportedConfig, parse_import};

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
    /// 用户为模块参数配置的值 `(key, value)`（键对应 `#!arguments=` 声明；旧清单缺省为空）。
    #[serde(default)]
    pub argument_values: Vec<(String, String)>,
    /// 资源图标 URL（可选；新建资源时可由嗅探结果预填；旧清单缺省为 `None`）。
    #[serde(default)]
    pub icon: Option<String>,
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
            argument_values: Vec::new(),
            icon: None,
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
    /// 本次导入配置头解析出的元数据（`#!key=value`）。
    pub meta: ConfigMeta,
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
    /// 各远程资源携带的配置头元数据（含 `#!arguments` 参数声明与默认值）。
    pub metas: Vec<ConfigMeta>,
}

/// 把 Surge/Loon 模块 `argument=` 模板中的 `{key}` / `{{{key}}}` 占位替换为具体值。
///
/// 优先级：用户配置值（`user_values`，key→值）→ 参数声明默认值（`defaults`，
/// key→默认值）→ 无则保留原占位符。`{{{key}}}`（Surge 标准三花括号占位）与 `{key}`
/// 两种形式均支持，且优先匹配长形式，避免简写替换污染三花括号占位。返回值即注入
/// JS `$argument` 的字符串。
pub fn resolve_argument_template(
    template: &str,
    user_values: &HashMap<String, String>,
    defaults: &HashMap<String, String>,
) -> String {
    // 用户值覆盖默认值（键重复时以用户配置为准）。
    let mut values: HashMap<String, String> = HashMap::new();
    values.extend(defaults.iter().map(|(k, v)| (k.clone(), v.clone())));
    values.extend(user_values.iter().map(|(k, v)| (k.clone(), v.clone())));

    let mut out = template.to_string();
    // 先替换长形式 `{{{key}}}`，再替换简写 `{key}`；未声明的占位（无值）保留原样。
    for (key, value) in &values {
        out = out.replace(&placeholder(key, true), value);
    }
    for (key, value) in &values {
        out = out.replace(&placeholder(key, false), value);
    }
    out
}

/// 构造占位符字面量：`triple=true` 生成 `{{{key}}}`（Surge 三花括号标准占位），
/// 否则生成 `{key}`（简写形式）。
fn placeholder(key: &str, triple: bool) -> String {
    let (open, close) = if triple { ("{{{", "}}}") } else { ("{", "}") };
    format!("{open}{key}{close}")
}

/// 对缓存合并后的脚本钩子规则做 argument 模板替换：`{key}` / `{{{key}}}` → 用户值 →
/// 参数声明默认值 → 保留原样（见 [`resolve_argument_template`]）。
///
/// `remotes` 提供用户配置的参数值（[`RemoteResource::argument_values`]），
/// `metas` 提供各资源 `#!arguments=` 声明的键与默认值（[`ConfigMeta::arguments`]）。
pub fn apply_argument_templates(
    rules: Vec<ScriptRule>,
    metas: &[ConfigMeta],
    remotes: &[RemoteResource],
) -> Vec<ScriptRule> {
    let mut user_values = HashMap::new();
    for remote in remotes {
        for (key, value) in &remote.argument_values {
            user_values.insert(key.clone(), value.clone());
        }
    }
    let mut defaults = HashMap::new();
    for meta in metas {
        for arg in &meta.arguments {
            defaults.insert(arg.key.clone(), arg.default_value.clone());
        }
    }
    rules
        .into_iter()
        .map(|mut rule| {
            if let Some(template) = rule.argument.take() {
                rule.argument = Some(resolve_argument_template(
                    &template,
                    &user_values,
                    &defaults,
                ));
            }
            rule
        })
        .collect()
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
            merged.metas.extend(part.metas);
            for hostname in part.hostnames {
                if !merged.hostnames.contains(&hostname) {
                    merged.hostnames.push(hostname);
                }
            }
        }
        Ok(merged)
    }

    /// 将一次导入的 [`ImportedConfig`] 合并进固定缓存 `remote_cache/imported.json`。
    ///
    /// 脚本钩子 / 定时任务的 `source` 为空（未拉取）时跳过并计入
    /// [`ImportSummary::warnings`]；重写规则与 hostname 直接合入。
    /// 重复导入在既有缓存上追加（不覆盖）。
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
        // 配置头元数据（含 `#!arguments` 声明）随缓存持久化，供运行时替换 argument 模板。
        if imported.meta != ConfigMeta::default() {
            cached.meta = Some(imported.meta.clone());
        }

        std::fs::create_dir_all(&cache_dir)?;
        let text = serde_json::to_string_pretty(&cached)?;
        std::fs::write(&cache_path, text)?;
        Ok(summary)
    }

    /// 导入一段三方配置内容并合并进本地缓存：
    /// `parse_import` → [`Self::fill_script_sources`]（拉取脚本源码）→ [`Self::merge_imported`]。
    ///
    /// 单个脚本 / 任务脚本拉取失败仅记入 [`ImportSummary::warnings`] 并跳过，
    /// 不阻塞重写规则与 hostname 的合入。返回摘要含配置头元数据与拉取统计。
    pub async fn import_content(
        &self,
        content: &str,
        dialect: ScriptDialect,
    ) -> PanelResult<ImportSummary> {
        let mut imported = parse_import(content, dialect)
            .map_err(|e| PanelError::Client(format!("imported config parse failed: {e}")))?;
        self.fill_script_sources(&mut imported).await;
        let mut summary = self.merge_imported(&imported)?;
        summary.meta = imported.meta;
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

    /// 回填 Snippet 中脚本钩子 / 定时任务的远端源码。
    ///
    /// 逐个拉取 [`ImportedConfig::script_urls`] 与 [`ImportedConfig::task_scripts`] 指向的
    /// 脚本，成功写回对应 `source`；失败记入 [`ImportedConfig::warnings`] 并丢弃该脚本，
    /// 不阻塞其他脚本与规则。
    pub async fn fill_script_sources(&self, imported: &mut ImportedConfig) {
        let scripts = std::mem::take(&mut imported.scripts);
        let script_urls = std::mem::take(&mut imported.script_urls);
        let mut kept_scripts = Vec::new();
        let mut kept_urls = Vec::new();
        for (rule, (name, url)) in scripts.into_iter().zip(script_urls) {
            match self.fetch_text(&url).await {
                Ok(source) => {
                    let mut rule = rule;
                    rule.source = source;
                    kept_scripts.push(rule);
                    kept_urls.push((name, url));
                }
                Err(e) => imported
                    .warnings
                    .push(format!("hook script fetch failed: {e}")),
            }
        }
        // 保持 scripts 与 script_urls 对齐（仅保留拉取成功的脚本）
        imported.scripts = kept_scripts;
        imported.script_urls = kept_urls;

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
    }

    /// 拉取 Snippet 片段：解析 → 逐个回填脚本钩子/任务源码（失败记 warning 跳过）。
    async fn fetch_snippet(&self, remote: &RemoteResource) -> PanelResult<ImportedConfig> {
        let text = self.fetch_text(&remote.url).await?;
        let mut imported = parse_import(&text, remote.dialect).map_err(|e| {
            PanelError::Client(format!("snippet '{}' parse failed: {e}", remote.name))
        })?;
        self.fill_script_sources(&mut imported).await;
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
    /// 配置头元数据（`#!icon` / `#!arguments` 等；旧缓存缺省为 `None`）。
    #[serde(default)]
    meta: Option<ConfigMeta>,
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
            meta: if cfg.meta != ConfigMeta::default() {
                Some(cfg.meta.clone())
            } else {
                None
            },
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
        if let Some(meta) = self.meta {
            merged.metas.push(meta);
        }
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
    /// 模块 `argument=` 模板（替换前原文；旧缓存缺省为 `None`）。
    #[serde(default)]
    argument: Option<String>,
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
            argument: rule.argument.clone(),
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
            argument: self.argument,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::import::ArgSpec;
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
                argument: None,
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

    /// 启动本地导入测试服务：提供 CamScanner 脚本与 404 端点。
    async fn spawn_import_server() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new()
            .route(
                "/camscanner.js",
                axum::routing::get(|| async { "const camscanner = 1;" }),
            )
            .route(
                "/missing.js",
                axum::routing::get(|| async { (StatusCode::NOT_FOUND, "not found") }),
            );
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[test]
    fn detect_resource_from_url_maps_suffixes_to_kind_and_dialect() {
        assert_eq!(
            detect_resource_from_url("https://example.com/config.sgmodule"),
            Some((RemoteKind::Snippet, ScriptDialect::Surge))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/conf.plugin"),
            Some((RemoteKind::Snippet, ScriptDialect::Loon))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.loon"),
            Some((RemoteKind::Snippet, ScriptDialect::Loon))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.conf"),
            Some((RemoteKind::Snippet, ScriptDialect::QuantumultX))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/script.js"),
            Some((RemoteKind::Script, ScriptDialect::QuantumultX))
        );
        // 带 query / fragment：后缀判定忽略 query
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.sgmodule?token=abc&x=1"),
            Some((RemoteKind::Snippet, ScriptDialect::Surge))
        );
        assert_eq!(
            detect_resource_from_url("https://example.com/script.js?token=abc#frag"),
            Some((RemoteKind::Script, ScriptDialect::QuantumultX))
        );
        // 大小写不敏感
        assert_eq!(
            detect_resource_from_url("https://example.com/RULES.SGMODULE"),
            Some((RemoteKind::Snippet, ScriptDialect::Surge))
        );
        // 无后缀 / 其他后缀 / 空串 → None
        assert_eq!(detect_resource_from_url("https://example.com/rules"), None);
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.txt"),
            None
        );
        assert_eq!(detect_resource_from_url(""), None);
        // 尾斜杠仍以最后一段文件名判定后缀（目录式 URL 视作片段）
        assert_eq!(
            detect_resource_from_url("https://example.com/rules.conf/"),
            Some((RemoteKind::Snippet, ScriptDialect::QuantumultX))
        );
    }

    #[tokio::test]
    async fn import_content_fills_script_sources_and_merges_surge_sgmodule() {
        let base = spawn_import_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let content = format!(
            "#!name=扫描全能王-解锁VIP\n\
             #!desc=扫描全能王-手机扫描仪 解锁黄金会员\n\
             #!date=2026-01-21\n\
             #!category=🐹 BOBO Premium\n\
             #!author=叮当猫chxm1023\n\
             #!icon=https://example.com/CamScanner.png\n\
             #!openUrl=https://apps.apple.com/app/id388627783\n\
             \n\
             [Script]\n\
             扫描全能王-解锁黄金会员 = type=http-response, pattern=https:\\/\\/api-cs\\.intsig\\.net\\/purchase\\/cs\\/query_property, script-path={base}/camscanner.js, requires-body=true, max-size=-1, timeout=60\n\
             \n\
             [MITM]\n\
             hostname = %APPEND% api-cs.intsig.net\n"
        );

        let summary = manager
            .import_content(&content, ScriptDialect::Surge)
            .await
            .unwrap();
        assert_eq!(summary.rewrites, 0);
        assert_eq!(summary.scripts, 1, "脚本 source 已回填，merge 不再跳过");
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.hostnames, 1);
        assert_eq!(summary.meta.name.as_deref(), Some("扫描全能王-解锁VIP"));
        assert_eq!(
            summary.meta.open_url.as_deref(),
            Some("https://apps.apple.com/app/id388627783")
        );
        assert!(
            !summary
                .warnings
                .iter()
                .any(|w| w.contains("source not fetched")),
            "不应有 source not fetched 警告: {:?}",
            summary.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].source, "const camscanner = 1;");
        assert_eq!(merged.scripts[0].name, "扫描全能王-解锁黄金会员");
        assert_eq!(merged.hostnames, vec!["api-cs.intsig.net".to_string()]);
    }

    #[tokio::test]
    async fn import_content_records_warning_on_script_fetch_failure_and_keeps_rewrites() {
        let base = spawn_import_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let content = format!(
            "#!name=失败导入\n\
             [rewrite_local]\n\
             ^https?://api-cs\\.intsig\\.net/purchase script-response-body {base}/missing.js\n\
             ^https?://example\\.com/ url-and-header https://target.example.com/\n\
             [mitm]\n\
             hostname = *.camscanner.com\n"
        );

        let summary = manager
            .import_content(&content, ScriptDialect::QuantumultX)
            .await
            .unwrap();
        assert_eq!(summary.rewrites, 1, "rewrite 不受脚本拉取失败影响");
        assert_eq!(summary.scripts, 0, "拉取失败的脚本被跳过");
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.hostnames, 1);
        assert_eq!(summary.meta.name.as_deref(), Some("失败导入"));
        assert!(
            summary.warnings.iter().any(|w| w.contains("missing.js")),
            "应有脚本拉取失败 warning: {:?}",
            summary.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.rewrites.len(), 1);
        assert!(merged.scripts.is_empty());
        assert!(merged.task_scripts.is_empty());
        assert_eq!(merged.hostnames, vec!["*.camscanner.com".to_string()]);
    }

    #[tokio::test]
    async fn import_content_handles_qx_conf_sample() {
        let base = spawn_import_server().await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        // QX 常见的 4 段式：`pattern url script-response-body <path>`（多余 `url` 修饰符忽略）
        let content = format!(
            "#!name=扫描全能王-QX\n\
             #!desc=QX 重写样例\n\
             [rewrite_local]\n\
             ^https:\\/\\/.*\\.(intsig\\.net|camscanner\\.com) url script-response-body {base}/camscanner.js\n\
             [mitm]\n\
             hostname = *.camscanner.com, *.intsig.net\n"
        );

        let summary = manager
            .import_content(&content, ScriptDialect::QuantumultX)
            .await
            .unwrap();
        assert_eq!(summary.rewrites, 0);
        assert_eq!(summary.scripts, 1, "脚本 source 已回填，merge 不再跳过");
        assert_eq!(summary.tasks, 0);
        assert_eq!(summary.hostnames, 2);
        assert_eq!(summary.meta.name.as_deref(), Some("扫描全能王-QX"));
        assert!(
            summary.warnings.is_empty(),
            "warnings: {:?}",
            summary.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].name, "hook-0");
        assert_eq!(merged.scripts[0].source, "const camscanner = 1;");
        assert_eq!(
            merged.hostnames,
            vec!["*.camscanner.com".to_string(), "*.intsig.net".to_string()]
        );
    }

    /// ④ `resolve_argument_template`：用户值优先 → 默认值 → 保留原样。
    #[test]
    fn resolve_argument_template_substitutes_user_values_then_defaults() {
        let user_values = HashMap::from([("token".to_string(), "abc".to_string())]);
        let defaults = HashMap::from([("server".to_string(), "api.example.com".to_string())]);
        assert_eq!(
            resolve_argument_template("{server}|{token}", &user_values, &defaults),
            "api.example.com|abc"
        );
        // 未声明的占位保留原样。
        assert_eq!(
            resolve_argument_template("{server}|{missing}", &user_values, &defaults),
            "api.example.com|{missing}"
        );
        // 无用户值、无默认值时原样保留。
        assert_eq!(
            resolve_argument_template("{server}", &HashMap::new(), &HashMap::new()),
            "{server}"
        );
    }

    /// `apply_argument_templates`：用户值优先于默认值，未声明占位保留。
    #[test]
    fn apply_argument_templates_prefers_user_values_over_defaults() {
        let rules = vec![
            ScriptRule {
                name: "r1".into(),
                kind: ScriptKind::HttpResponse,
                pattern: Regex::new(".*").unwrap(),
                requires_body: false,
                max_size: 131072,
                source: String::new(),
                argument: Some("{server}|{token}|{extra}".to_string()),
            },
            ScriptRule {
                name: "r2".into(),
                kind: ScriptKind::HttpRequest,
                pattern: Regex::new(".*").unwrap(),
                requires_body: false,
                max_size: 131072,
                source: String::new(),
                argument: None,
            },
        ];
        let metas = vec![ConfigMeta {
            arguments: vec![
                ArgSpec {
                    key: "server".into(),
                    default_value: "api.example.com".into(),
                    description: None,
                },
                ArgSpec {
                    key: "token".into(),
                    default_value: "default-token".into(),
                    description: None,
                },
            ],
            ..ConfigMeta::default()
        }];
        let remotes = vec![RemoteResource {
            argument_values: vec![("token".to_string(), "abc".to_string())],
            ..RemoteResource::default()
        }];

        let out = apply_argument_templates(rules, &metas, &remotes);
        assert_eq!(
            out[0].argument.as_deref(),
            Some("api.example.com|abc|{extra}")
        );
        // argument 为 None 的规则原样保留。
        assert_eq!(out[1].argument, None);
    }

    /// `resolve_argument_template` 支持 Surge 标准三花括号占位 `{{{key}}}`（优先匹配
    /// 长形式），同时兼容简写 `{key}`；用户值优先于默认值，未声明占位保留原样。
    #[test]
    fn resolve_argument_template_supports_triple_brace_placeholders() {
        let user_values = HashMap::from([("per_filter_video".to_string(), "1".to_string())]);
        let defaults = HashMap::from([("per_filter_video".to_string(), "0".to_string())]);

        // 三花括号占位：无用户值 → 默认值 0。
        assert_eq!(
            resolve_argument_template(
                "per_filter_video_thread={{{per_filter_video}}}",
                &HashMap::new(),
                &defaults,
            ),
            "per_filter_video_thread=0"
        );
        // 三花括号占位：用户值覆盖默认值。
        assert_eq!(
            resolve_argument_template(
                "per_filter_video_thread={{{per_filter_video}}}",
                &user_values,
                &defaults,
            ),
            "per_filter_video_thread=1"
        );
        // 长形式优先：`{{{a}}}` 整体替换，不被简写 `{a}` 部分污染。
        assert_eq!(
            resolve_argument_template(
                "{{{a}}}|{a}",
                &HashMap::new(),
                &HashMap::from([("a".to_string(), "X".to_string())]),
            ),
            "X|X"
        );
        // 未声明的三花括号占位保留原样。
        assert_eq!(
            resolve_argument_template("{{{missing}}}", &HashMap::new(), &HashMap::new(),),
            "{{{missing}}}"
        );
        // 简写与三花括号共存。
        assert_eq!(
            resolve_argument_template(
                "{server}|{{{token}}}",
                &HashMap::from([("token".to_string(), "abc".to_string())]),
                &HashMap::from([("server".to_string(), "api.example.com".to_string())]),
            ),
            "api.example.com|abc"
        );
    }

    /// `RemoteResource` 新增字段（argument_values / icon）经 save → load 往返保留；
    /// 旧清单缺省这些字段（serde default）。
    #[test]
    fn remote_resource_argument_values_and_icon_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "args".into(),
            url: "http://example.com/mod.sgmodule".into(),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::Surge,
            argument_values: vec![
                ("server".to_string(), "api.example.com".to_string()),
                ("token".to_string(), "abc".to_string()),
            ],
            icon: Some("https://example.com/icon.png".to_string()),
            ..RemoteResource::default()
        }];
        manager.save(&remotes).unwrap();
        assert_eq!(manager.load().unwrap(), remotes);

        // 旧清单（无新字段）读取回退为默认值，不报错。
        std::fs::write(
            manager.remotes_file(),
            r#"[{"name":"old","url":"http://example.com/x.js","kind":"Script","dialect":"Surge","update_interval_secs":86400,"enabled":true}]"#,
        )
        .unwrap();
        let loaded = manager.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].argument_values.is_empty());
        assert_eq!(loaded[0].icon, None);
    }

    /// `#!arguments` 声明与 `argument=` 模板经 fetch → cache → load 往返不丢失，
    /// meta 透出到 `MergedRemoteConfig.metas`。
    #[tokio::test]
    async fn snippet_arguments_and_meta_roundtrip_through_cache() {
        let base = spawn_remote_server(|base| {
            format!(
                "#!name=参数模块\n\
                 #!icon=https://example.com/icon.png\n\
                 #!arguments= server:api.example.com, token:default-token\n\
                 #!arguments-desc= {{server:\"API 服务器\", token:\"鉴权令牌\"}}\n\
                 \n\
                 [Script]\n\
                 xxx = type=http-response, pattern=^https://api\\.example\\.com/, script-path={base}/hook.js, argument={{server}}|{{token}}\n"
            )
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "args".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::Surge,
            ..RemoteResource::default()
        }];

        let report = manager.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1);
        assert!(
            report.warnings.is_empty(),
            "warnings: {:?}",
            report.warnings
        );

        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(
            merged.scripts[0].argument.as_deref(),
            Some("{server}|{token}")
        );
        assert_eq!(merged.metas.len(), 1);
        let meta = &merged.metas[0];
        assert_eq!(meta.name.as_deref(), Some("参数模块"));
        assert_eq!(meta.icon.as_deref(), Some("https://example.com/icon.png"));
        assert_eq!(meta.arguments.len(), 2);
        let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
        assert_eq!(server.default_value, "api.example.com");
        assert_eq!(server.description.as_deref(), Some("API 服务器"));
    }

    /// ①③④ BaiDuTieBa 真实样例经 fetch → cache → load → apply 全链路：朴素 desc、
    /// 数字布尔 requires-body、无限制 max-size、三花括号占位替换为默认值/用户值。
    #[tokio::test]
    async fn snippet_badubatieba_sample_roundtrip_and_argument_resolution() {
        // `argument=` 模板原文含三花括号，用字面量避免 format 转义。
        let arg_tpl = "per_filter_video_thread={{{per_filter_video}}}";
        let base = spawn_remote_server(move |base| {
            format!(
                "#!arguments=per_filter_video:0\n\
                 #!arguments-desc=per_filter_video:设置为1则推荐页不展示视频贴\n\
                 \n\
                 [Script]\n\
                 贴吧proto = type=http-response,pattern=^https?:\\/\\/(tiebac|c\\.tieba)\\.baidu\\.com\\/...$ ,requires-body=1,binary-body-mode=1,max-size=-1,script-path={base}/hook.js,argument={arg_tpl}\n"
            )
        })
        .await;
        let dir = tempfile::tempdir().unwrap();
        let manager = RemoteManager::new(dir.path().to_path_buf());
        let remotes = vec![RemoteResource {
            name: "baidu".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::Surge,
            ..RemoteResource::default()
        }];

        let report = manager.fetch_all(&remotes).await;
        assert_eq!(report.fetched, 1);
        assert!(
            report.warnings.is_empty(),
            "warnings: {:?}",
            report.warnings
        );

        // meta：朴素 desc 与默认值合并进 ArgSpec。
        let merged = manager.load_cached().unwrap();
        assert_eq!(merged.metas.len(), 1);
        let spec = &merged.metas[0].arguments[0];
        assert_eq!(spec.key, "per_filter_video");
        assert_eq!(spec.default_value, "0");
        assert_eq!(
            spec.description.as_deref(),
            Some("设置为1则推荐页不展示视频贴")
        );

        // 脚本钩子：requires-body=1 / max-size=-1 / 三花括号占位原文。
        assert_eq!(merged.scripts.len(), 1);
        assert_eq!(merged.scripts[0].name, "贴吧proto");
        assert!(merged.scripts[0].requires_body);
        assert_eq!(merged.scripts[0].max_size, 10 * 1024 * 1024);
        assert_eq!(
            merged.scripts[0].argument.as_deref(),
            Some("per_filter_video_thread={{{per_filter_video}}}")
        );

        // apply_argument_templates：无用户值 → 默认值 0。
        let resolved = apply_argument_templates(merged.scripts, &merged.metas, &remotes);
        assert_eq!(
            resolved[0].argument.as_deref(),
            Some("per_filter_video_thread=0")
        );

        // 用户配置值 1 → 覆盖默认值。
        let remotes_with_value = vec![RemoteResource {
            name: "baidu".into(),
            url: format!("{base}/snippet"),
            kind: RemoteKind::Snippet,
            dialect: ScriptDialect::Surge,
            argument_values: vec![("per_filter_video".to_string(), "1".to_string())],
            ..RemoteResource::default()
        }];
        let merged2 = manager.load_cached().unwrap();
        let resolved2 =
            apply_argument_templates(merged2.scripts, &merged2.metas, &remotes_with_value);
        assert_eq!(
            resolved2[0].argument.as_deref(),
            Some("per_filter_video_thread=1")
        );
    }
}
