//! 三方配置片段导入（QX / Surge / Loon → pp-mitm / pp-script 规则）。
//!
//! 这是「远程订阅（2.3b）」与「配置导入（2.4）」的共同地基：把 Quantumult X /
//! Surge / Loon 的 rewrite / script / task / mitm 配置片段解析为
//! pp-mitm 的 [`RewriteRule`] / 脚本钩子规则与 pp-script 的 [`TaskScript`]。
//!
//! 设计取舍：
//! - 只覆盖常用子集，未知行跳过并记入 [`ImportedConfig::warnings`]（注释/空行静默跳过）；
//! - 脚本一律视为远端 http(s) URL：`source` 留空，URL 记入
//!   [`ImportedConfig::script_urls`] / [`ImportedConfig::task_scripts`]，由调用方拉取后回填；
//! - pp-mitm 字段与生态语法结构性不符时用现有字段近似并记 warning（不改 pp-mitm）。

use std::collections::HashMap;

use pp_common::PanelResult;
use pp_mitm::{Phase, RewriteKind, RewriteRule, ScriptRule as HookScriptRule};
use pp_script::{ScriptDialect, ScriptKind, TaskScript};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// 一次导入解析的结果。
#[derive(Default)]
pub struct ImportedConfig {
    /// URL / Header / Body 重写与 Reject / Mock 规则。
    pub rewrites: Vec<RewriteRule>,
    /// 脚本钩子规则；`source` 为空，对应脚本 URL 见 [`ImportedConfig::script_urls`]。
    pub scripts: Vec<HookScriptRule>,
    /// 脚本钩子远端地址 `(脚本名, URL)`，与 `scripts` 按顺序一一对应。
    pub script_urls: Vec<(String, String)>,
    /// 定时任务（`TaskScript.source` 为空）与其脚本 URL。
    pub task_scripts: Vec<(TaskScript, String)>,
    /// MITM hostname 白名单。
    pub hostnames: Vec<String>,
    /// 未识别行 / 无法表达的映射偏差。
    pub warnings: Vec<String>,
    /// 文件头 `#!key=value` 元数据（见 [`ConfigMeta`]）。
    pub meta: ConfigMeta,
}

impl ImportedConfig {
    /// 记录一条 warning：`tracing::warn` 的同时写入 `warnings` 列表。
    fn warn(&mut self, section: &str, line: &str, msg: &str) {
        let text = format!("[{section}] {msg}: {line}");
        tracing::warn!(section, "{text}");
        self.warnings.push(text);
    }
}

/// 配置文件头 `#!key=value` 元数据（Surge `.sgmodule` / QX `.conf` / Loon 常见）。
///
/// 各字段均为 `Option`：头中缺失的键保持 `None`；`openUrl` 等 camelCase 键归一化为
/// snake_case 字段。`#[serde(default)]` 保证缺失键在序列化往返中回退为 `None`。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigMeta {
    /// `#!name`：配置名。
    pub name: Option<String>,
    /// `#!desc`（别名 `#!description`）：配置描述；两者并存时 `#!desc` 优先不覆盖。
    pub desc: Option<String>,
    /// `#!author`：作者。
    pub author: Option<String>,
    /// `#!icon`：图标 URL。
    pub icon: Option<String>,
    /// `#!date`：发布日期。
    pub date: Option<String>,
    /// `#!category`：分类标签。
    pub category: Option<String>,
    /// `#!openUrl`：关联链接（如 App Store）。
    pub open_url: Option<String>,
    /// `#!arguments=` / `#!arguments-desc=` 声明的模块参数（键/默认值/描述）。
    pub arguments: Vec<ArgSpec>,
}

/// Surge/Loon 模块 `#!arguments=` 声明的单个参数。
///
/// `argument="{key}"` 模板占位在运行时按「用户值 → 默认值 → 保留原样」替换。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgSpec {
    /// 参数键（模板占位 `{key}` 使用，`#!arguments= key:default`）。
    pub key: String,
    /// 默认值（用户未配置该键时替换进 argument 模板；可为空串）。
    pub default_value: String,
    /// 参数描述（`#!arguments-desc=` 提供；可选）。
    pub description: Option<String>,
}

/// 解析文件头连续 `#!key=value` 元数据行；遇到首个非 `#!` 行（含空行）即停止。
///
/// 未知键与格式异常行静默忽略（元数据不影响规则解析）。
pub fn parse_config_meta(content: &str) -> ConfigMeta {
    let mut meta = ConfigMeta::default();
    for raw in content.lines() {
        let line = raw.trim();
        if !line.starts_with("#!") {
            break;
        }
        let Some((key, value)) = line[2..].trim().split_once('=') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() {
            continue;
        }
        match key.trim().to_ascii_lowercase().as_str() {
            "name" => meta.name = Some(value.to_string()),
            "desc" => meta.desc = Some(value.to_string()),
            "description" => {
                // `#!description=` 是 `#!desc=` 的别名；`#!desc=` 已填充（或先出现）时保持 `desc` 优先。
                if meta.desc.is_none() {
                    meta.desc = Some(value.to_string());
                }
            }
            "author" => meta.author = Some(value.to_string()),
            "icon" => meta.icon = Some(value.to_string()),
            "date" => meta.date = Some(value.to_string()),
            "category" => meta.category = Some(value.to_string()),
            "openurl" => meta.open_url = Some(value.to_string()),
            "arguments" => meta.arguments = parse_arguments_decl(value),
            "arguments-desc" => merge_argument_descriptions(&mut meta.arguments, value),
            _ => {} // 未知键忽略
        }
    }
    meta
}

/// 解析 `#!arguments= key:default, key2:default2` 声明：逗号分隔，值可含空格（trim）。
///
/// 逗号切分是引号感知的（`Types:"Translate,External"` 内的逗号不切分）；
/// 值可带引号（`key:"default"`，去引号）或裸值（`key:default`）。无 `:` 的段
/// （如 `#!arguments= foo`）静默跳过（无默认值无从声明）。
fn parse_arguments_decl(value: &str) -> Vec<ArgSpec> {
    let mut args = Vec::new();
    for pair in split_kv_segments(value) {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((key, default_value)) = pair.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        args.push(ArgSpec {
            key: key.to_string(),
            default_value: strip_quotes(default_value.trim()).to_string(),
            description: None,
        });
    }
    args
}

/// 解析 `#!arguments-desc= {key:"描述", ...}` 并按 key 合并进参数声明列表。
///
/// 先试严格 JSON 对象；失败则按 `key:"value"` / `key:'value'` 语法宽松提取。
/// 描述中出现的、声明中缺失的键以空默认值补入 [`ArgSpec`]。
fn merge_argument_descriptions(args: &mut Vec<ArgSpec>, value: &str) {
    let pairs = parse_desc_pairs(value);
    for (key, desc) in pairs {
        if let Some(spec) = args.iter_mut().find(|a| a.key == key) {
            spec.description = Some(desc);
        } else {
            args.push(ArgSpec {
                key,
                default_value: String::new(),
                description: Some(desc),
            });
        }
    }
}

/// 提取 `#!arguments-desc=` 的 `(key, desc)` 键值对（严格 JSON → 宽松语法 → 朴素语法回退）。
fn parse_desc_pairs(value: &str) -> Vec<(String, String)> {
    // 1. 严格 JSON 对象：`{"key": "desc", ...}`。
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(value) {
        if let Some(obj) = json.as_object() {
            return obj
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect();
        }
    }
    // 2. 宽松语法：`key:"描述"` / `key:'描述'`（键未加引号、值含空格/中文均可）。
    let mut out = Vec::new();
    let Ok(re) = Regex::new(r#"([A-Za-z0-9_.\-]+)\s*:\s*("([^"]*)"|'([^']*)')"#) else {
        return out;
    };
    for caps in re.captures_iter(value) {
        let key = caps[1].trim().to_string();
        if key.is_empty() {
            continue;
        }
        let desc = caps
            .get(2)
            .map(|m| m.as_str())
            .map(|quoted| quoted[1..quoted.len().saturating_sub(1)].to_string())
            .unwrap_or_default();
        out.push((key, desc));
    }
    if !out.is_empty() {
        return out;
    }
    // 3. 朴素语法：`key:描述`（无 `{}`、无引号，描述可含空格/中文，逗号分隔多个）。
    //    `regex` crate 不支持 look-around，故手工扫描：仅在逗号后紧跟 `key:` 时才切分，
    //    避免描述内的逗号误切。
    parse_naive_desc_pairs(value)
}

/// 朴素 `#!arguments-desc=` 语法 `key:描述[, key2:描述2, ...]` 的键值对提取。
///
/// 描述无引号、可含中文/空格/冒号/逗号；仅在逗号后紧跟合法键 + 冒号的位置切分新段。
fn parse_naive_desc_pairs(value: &str) -> Vec<(String, String)> {
    fn is_valid_key(s: &str) -> bool {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
    }

    let mut out = Vec::new();
    let mut rest = value.trim();
    loop {
        let seg = rest.trim_start();
        if seg.is_empty() {
            break;
        }
        let Some(colon) = seg.find(':') else { break };
        let key = &seg[..colon];
        if !is_valid_key(key) {
            break;
        }
        let desc_src = &seg[colon + 1..];
        // 定位描述终点：下一个逗号且其后紧跟 `key:` 的位置；无则延伸到末尾。
        let mut seg_end = desc_src.len();
        let mut next_start: Option<usize> = None;
        for (offset, c) in desc_src.char_indices() {
            if c == ',' {
                let after = desc_src[offset + 1..].trim_start();
                if let Some(c2) = after.find(':') {
                    if is_valid_key(&after[..c2]) {
                        seg_end = offset;
                        next_start = Some(offset + 1);
                        break;
                    }
                }
            }
        }
        let desc = desc_src[..seg_end].trim();
        out.push((key.to_string(), desc.to_string()));
        match next_start {
            Some(ns) => rest = &desc_src[ns..],
            None => break,
        }
    }
    out
}

/// 解析 QX / Surge / Loon 配置片段。
///
/// `dialect` 由调用方指定来源软件，决定 [`TaskScript::dialect`] 等方言标记；
/// 各软件同构语法（如 Surge / Loon 的 `[Script]`）共用同一解析路径。
pub fn parse_import(content: &str, dialect: ScriptDialect) -> PanelResult<ImportedConfig> {
    let mut cfg = ImportedConfig {
        meta: parse_config_meta(content),
        ..ImportedConfig::default()
    };
    let mut section = String::new();
    let mut hook_index = 0usize;

    for raw in content.lines() {
        let line = raw.trim();
        if is_comment_or_blank(line) {
            continue;
        }
        if let Some(name) = section_name(line) {
            section = name;
            continue;
        }
        match section.as_str() {
            "rewrite_local" | "rewrite_remote" => parse_qx_rewrite(&mut cfg, &mut hook_index, line),
            "task_local" => parse_qx_task(&mut cfg, dialect, line),
            "mitm" => parse_mitm_hostnames(&mut cfg, line),
            "script" => parse_surge_script(&mut cfg, dialect, line),
            "url rewrite" => parse_surge_url_rewrite(&mut cfg, line),
            "header rewrite" => parse_surge_header_rewrite(&mut cfg, line),
            "map local" => parse_surge_map_local(&mut cfg, line),
            // 其他 section（如 QX `[task_remote]` / Surge `[General]`）整段跳过。
            _ => {}
        }
    }

    Ok(cfg)
}

/// 注释（`#` / `;` 开头）或空白行。
fn is_comment_or_blank(line: &str) -> bool {
    line.is_empty() || line.starts_with('#') || line.starts_with(';')
}

/// 解析 `[section]` 头，归一化为小写 + 单词间单空格（`[URL Rewrite]` → `"url rewrite"`）。
fn section_name(line: &str) -> Option<String> {
    let t = line.trim();
    if t.len() >= 2 && t.starts_with('[') && t.ends_with(']') {
        let inner = &t[1..t.len() - 1];
        let normalized = inner
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        Some(normalized)
    } else {
        None
    }
}

/// 编译正则；失败时记 warning 并返回 `None`（不 panic）。
fn compile_pattern(
    src: &str,
    cfg: &mut ImportedConfig,
    section: &str,
    line: &str,
) -> Option<Regex> {
    match Regex::new(src) {
        Ok(re) => Some(re),
        Err(e) => {
            cfg.warn(section, line, &format!("invalid regex: {e}"));
            None
        }
    }
}

/// 判断是否为远端 http(s) 脚本 URL；其余（本地路径等）不支持。
fn is_remote_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// 从脚本 URL 派生任务名：取路径末段文件名（去掉 `.js` 后缀），失败时回退整个 URL。
fn derive_name_from_url(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url) {
        if let Some(seg) = parsed.path_segments().and_then(|mut it| it.next_back()) {
            if !seg.is_empty() {
                let stem = seg.strip_suffix(".js").unwrap_or(seg);
                return stem.to_string();
            }
        }
    }
    url.to_string()
}

/// 去掉首尾成对双引号（`"* * * * *"` → `* * * * *`）；仅当首尾均为引号时剥离
/// （`Types="a"&Vendor="b"` 这类整体未加引号的值保持原样）。
fn strip_quotes(s: &str) -> &str {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

/// 解析 `key=value` 值（`true` / `1` / `yes` 视为真，默认假）。
///
/// Surge 的 `requires-body` 同时接受布尔与数字形式：`true/false` 与 `1/0` 均正确映射。
fn parse_bool(v: Option<&String>) -> bool {
    matches!(
        v.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("true") | Some("1") | Some("yes")
    )
}

/// Surge `max-size` 无限制值映射的近似上限（10MB）。
///
/// Surge 语义中 `max-size=-1` / `max-size=0` 表示「不限制 body 大小」；pp-mitm 的
/// `max_size` 为 `usize`，直接映射 `usize::MAX` 会破坏内部缓冲语义，故折中映射为
/// 10MB 上限（足够承载绝大多数真实响应体）。
const MAX_SIZE_UNLIMITED: usize = 10 * 1024 * 1024;

/// 解析 Surge `max-size`：`-1` / `0`（unlimited）→ [`MAX_SIZE_UNLIMITED`]，正常数字原样解析。
fn parse_max_size(v: Option<&String>) -> Option<usize> {
    let s = v?.trim();
    match s {
        "-1" | "0" => Some(MAX_SIZE_UNLIMITED),
        s => s.parse::<usize>().ok(),
    }
}

/// 按逗号切分 `key=value` / `key:value` 段，但跳过引号内与 `[` `]` / `{` `}` 内嵌的逗号
/// （`Types:"Translate,External"`、`argument=[{a},{b}]`、正则量词 `\d{1,2}` 均不误切）。
fn split_kv_segments(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut depth = 0i32;
    for c in input.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            '[' | '{' if !in_quotes => {
                depth += 1;
                current.push(c);
            }
            ']' | '}' if !in_quotes => {
                depth -= 1;
                current.push(c);
            }
            ',' if !in_quotes && depth == 0 => {
                out.push(std::mem::take(&mut current));
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// 解析逗号分隔的 `key=value` 参数列表；key 归一化为小写，value 去引号。
fn parse_kv_params(input: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in split_kv_segments(input) {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        let Some((k, v)) = pair.split_once('=') else {
            continue;
        };
        map.insert(k.trim().to_lowercase(), strip_quotes(v.trim()).to_string());
    }
    map
}

/// 按空白分词，但保留双引号内嵌空白的整体（如 `data="hello world"`）。
fn split_tokens_keep_quoted(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in line.chars() {
        match c {
            '"' => {
                in_quotes = !in_quotes;
                current.push(c);
            }
            c if c.is_whitespace() && !in_quotes => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// QX `[rewrite_local]` / `[rewrite_remote]` 行解析。
///
/// | 输入 | 内部规则 | 偏差 |
/// |------|----------|------|
/// | `pattern url-and-header target` | `UrlRewrite` | header 部分丢弃 |
/// | `pattern url-307/url-302 target` | `UrlRewrite` | 重定向状态码丢失（记录偏差） |
/// | `pattern script-response-body path` | `ScriptRule{HttpResponse}` | body 上限固定 131072 |
/// | `pattern script-request-body path` | `ScriptRule{HttpRequest}` | 同上 |
/// | `pattern script-echo-response path` | `ScriptRule{HttpResponse, no body}` | 同上 |
/// | `pattern reject/reject-200/reject-dict` | `Reject` | 拒绝状态码/内容丢失 |
/// | `pattern url-response-body regex repl` | `BodyRewrite{Response}` | body 正则丢失（记录偏差） |
/// | `pattern url-request-header regex repl` | `BodyRewrite{Request}` | 语义近似（记录偏差） |
///
/// `[rewrite_remote]` 中的远端引用行（`url, tag=...`）无法内联解析，记入 warnings。
fn parse_qx_rewrite(cfg: &mut ImportedConfig, hook_index: &mut usize, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        cfg.warn("rewrite", line, "unrecognized line");
        return;
    }
    // 兼容部分生态写法 `pattern url script-response-body <path>`（多余的 `url` 修饰符忽略）
    let (pattern_src, action, args) = if tokens.len() >= 3 && tokens[1] == "url" {
        (tokens[0], tokens[2], &tokens[3..])
    } else {
        (tokens[0], tokens[1], &tokens[2..])
    };
    let Some(pattern) = compile_pattern(pattern_src, cfg, "rewrite", line) else {
        return;
    };
    match action {
        "url-and-header" | "url-307" | "url-302" => {
            let Some(target) = args.first() else {
                cfg.warn("rewrite", line, "missing rewrite target");
                return;
            };
            if action != "url-and-header" {
                cfg.warn(
                    "rewrite",
                    line,
                    &format!(
                        "{action} redirect status cannot be expressed, approximated as UrlRewrite"
                    ),
                );
            }
            cfg.rewrites.push(RewriteRule {
                pattern,
                kind: RewriteKind::UrlRewrite {
                    target: (*target).to_string(),
                },
            });
        }
        "script-response-body" | "script-request-body" | "script-echo-response" => {
            let Some(path) = args.first() else {
                cfg.warn("rewrite", line, "missing script path");
                return;
            };
            let (kind, requires_body) = match action {
                "script-request-body" => (ScriptKind::HttpRequest, true),
                "script-echo-response" => (ScriptKind::HttpResponse, false),
                _ => (ScriptKind::HttpResponse, true),
            };
            let name = format!("hook-{hook_index}");
            *hook_index += 1;
            if is_remote_url(path) {
                cfg.script_urls.push((name.clone(), (*path).to_string()));
            } else {
                cfg.warn("rewrite", line, "local script path not supported, skipped");
            }
            // QX script 行同样支持 `argument={key}` 附加参数（如 rewrite_remote 的
            // `..., argument=xxx` 风格；rewrite_local 亦可尾随空格分隔的 argument=）。
            let argument = args
                .get(1..)
                .map(|rest| parse_kv_params(&rest.join(" ")))
                .and_then(|params| params.get("argument").cloned());
            cfg.scripts.push(HookScriptRule {
                name,
                kind,
                pattern,
                requires_body,
                max_size: 131072,
                source: String::new(),
                argument,
            });
        }
        "reject" | "reject-200" | "reject-dict" => {
            cfg.rewrites.push(RewriteRule {
                pattern,
                kind: RewriteKind::Reject,
            });
        }
        "url-response-body" | "url-request-header" => {
            let Some(regex_target) = args.first() else {
                cfg.warn("rewrite", line, "missing body regex");
                return;
            };
            let Some(replacement) = args.get(1) else {
                cfg.warn("rewrite", line, "missing replacement");
                return;
            };
            let phase = if action == "url-response-body" {
                Phase::Response
            } else {
                Phase::Request
            };
            cfg.warn(
                "rewrite",
                line,
                &format!(
                    "{action} body regex '{regex_target}' cannot be expressed (pp-mitm couples URL gate \
                     and body replace into one pattern); URL pattern kept as gate"
                ),
            );
            cfg.rewrites.push(RewriteRule {
                pattern,
                kind: RewriteKind::BodyRewrite {
                    phase,
                    replacement: (*replacement).to_string(),
                },
            });
        }
        other => cfg.warn("rewrite", line, &format!("unrecognized action '{other}'")),
    }
}

/// QX `[task_local]` 行解析：`<5段cron> <script-url>[, tag=..., img-url=...]`。
///
/// pp-script 的 cron crate 需要 6 段，解析时前缀补 `"0 "`；`name` 取 `tag`，
/// 缺失时从 URL 派生。
fn parse_qx_task(cfg: &mut ImportedConfig, dialect: ScriptDialect, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 6 {
        cfg.warn(
            "task",
            line,
            "unrecognized line (expected cron + script url)",
        );
        return;
    }
    let cron_5 = tokens[..5].join(" ");
    let rest = tokens[5..].join(" ");
    let (url, params) = rest.split_once(',').unwrap_or((rest.as_str(), ""));
    let url = url.trim();
    if !is_remote_url(url) {
        cfg.warn(
            "task",
            line,
            "script url missing or not a remote http(s) url",
        );
        return;
    }
    let mut tag: Option<String> = None;
    for pair in params.split(',') {
        if let Some((k, v)) = pair.trim().split_once('=') {
            if k.trim().eq_ignore_ascii_case("tag") {
                tag = Some(strip_quotes(v.trim()).to_string());
            }
        }
    }
    let name = tag
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| derive_name_from_url(url));
    let cron_expr = format!("0 {cron_5}");
    cfg.task_scripts.push((
        TaskScript {
            name,
            cron_expr,
            source: String::new(),
            dialect,
            enabled: true,
        },
        url.to_string(),
    ));
}

/// `[mitm]` / `[MITM]` hostname 行解析：`hostname = a, b, -exclude`。
///
/// `-` / `!` 前缀为排除项，pp-mitm 仅支持白名单，记 warning 后跳过；
/// Surge 的 `%APPEND%` 前缀直接剥离。
fn parse_mitm_hostnames(cfg: &mut ImportedConfig, line: &str) {
    let Some(eq) = line.find('=') else {
        cfg.warn(
            "mitm",
            line,
            "unrecognized line (expected 'hostname = ...')",
        );
        return;
    };
    for entry in line[eq + 1..].split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let entry = entry
            .strip_prefix("%APPEND%")
            .map(str::trim)
            .unwrap_or(entry);
        if entry.starts_with('-') || entry.starts_with('!') {
            cfg.warn(
                "mitm",
                line,
                &format!("exclude pattern '{entry}' not supported, skipped"),
            );
            continue;
        }
        cfg.hostnames.push(entry.to_string());
    }
}

/// Surge / Loon `[Script]` 行解析：`name = type=...,pattern=...,script-path=...`。
///
/// | type | 内部规则 | 说明 |
/// |------|----------|------|
/// | `http-response` | `ScriptRule{HttpResponse}` | 默认 `requires-body=false`、`max-size=131072` |
/// | `http-request` | `ScriptRule{HttpRequest}` | 同上 |
/// | `cron` | `TaskScript` | `cronexp` 5 段前缀补 `"0 "` 变 6 段 |
///
/// Loon 与 Surge 的参数名差异（`http-types` / `require-body`）按别名兼容。
fn parse_surge_script(cfg: &mut ImportedConfig, dialect: ScriptDialect, line: &str) {
    let Some(eq) = line.find('=') else {
        cfg.warn(
            "script",
            line,
            "unrecognized line (expected 'name = type=...')",
        );
        return;
    };
    let name = line[..eq].trim();
    if name.is_empty() {
        cfg.warn("script", line, "empty script name");
        return;
    }
    let params = parse_kv_params(&line[eq + 1..]);
    let Some(type_val) = params
        .get("type")
        .or_else(|| params.get("http-types"))
        .map(String::as_str)
    else {
        cfg.warn("script", line, "missing 'type' parameter");
        return;
    };
    match type_val {
        "http-response" | "http-request" => {
            let Some(pattern_src) = params.get("pattern") else {
                cfg.warn("script", line, "missing 'pattern' parameter");
                return;
            };
            let Some(script_path) = params.get("script-path") else {
                cfg.warn("script", line, "missing 'script-path' parameter");
                return;
            };
            let Some(pattern) = compile_pattern(pattern_src, cfg, "script", line) else {
                return;
            };
            if !is_remote_url(script_path) {
                cfg.warn("script", line, "local script path not supported, skipped");
                return;
            }
            let kind = if type_val == "http-request" {
                ScriptKind::HttpRequest
            } else {
                ScriptKind::HttpResponse
            };
            let requires_body = parse_bool(
                params
                    .get("requires-body")
                    .or_else(|| params.get("require-body")),
            );
            // `max-size=-1` / `0`（Surge unlimited）映射为 10MB 上限，见 [`parse_max_size`]。
            let max_size = parse_max_size(params.get("max-size")).unwrap_or(131072);
            // Surge/Loon 脚本行参数 `argument={key}|...`（模板占位运行时替换）。
            let argument = params.get("argument").cloned();
            cfg.script_urls
                .push((name.to_string(), script_path.clone()));
            cfg.scripts.push(HookScriptRule {
                name: name.to_string(),
                kind,
                pattern,
                requires_body,
                max_size,
                source: String::new(),
                argument,
            });
        }
        "cron" => {
            let Some(cron5) = params.get("cronexp") else {
                cfg.warn("script", line, "missing 'cronexp' parameter");
                return;
            };
            let Some(script_path) = params.get("script-path") else {
                cfg.warn("script", line, "missing 'script-path' parameter");
                return;
            };
            if !is_remote_url(script_path) {
                cfg.warn("script", line, "local script path not supported, skipped");
                return;
            }
            let cron_expr = format!("0 {}", cron5.trim());
            cfg.task_scripts.push((
                TaskScript {
                    name: name.to_string(),
                    cron_expr,
                    source: String::new(),
                    dialect,
                    enabled: true,
                },
                script_path.clone(),
            ));
        }
        other => cfg.warn(
            "script",
            line,
            &format!("unrecognized script type '{other}'"),
        ),
    }
}

/// Surge / Loon `[URL Rewrite]` 行解析：`pattern target [header-arg]` → `UrlRewrite`。
///
/// 第三段 request-header 参数无法表达，记偏差。
fn parse_surge_url_rewrite(cfg: &mut ImportedConfig, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        cfg.warn(
            "url rewrite",
            line,
            "unrecognized line (expected 'pattern target')",
        );
        return;
    }
    let Some(pattern) = compile_pattern(tokens[0], cfg, "url rewrite", line) else {
        return;
    };
    if tokens.len() > 2 {
        cfg.warn(
            "url rewrite",
            line,
            "request-header argument cannot be expressed, only URL rewritten",
        );
    }
    cfg.rewrites.push(RewriteRule {
        pattern,
        kind: RewriteKind::UrlRewrite {
            target: tokens[1].to_string(),
        },
    });
}

/// Surge / Loon `[Header Rewrite]` 行解析：
/// `pattern header-replace Name Value` / `pattern header-del Name` → `HeaderRewrite{Request}`。
fn parse_surge_header_rewrite(cfg: &mut ImportedConfig, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 3 {
        cfg.warn("header rewrite", line, "unrecognized line");
        return;
    }
    let Some(pattern) = compile_pattern(tokens[0], cfg, "header rewrite", line) else {
        return;
    };
    match tokens[1] {
        "header-replace" => {
            if tokens.len() < 4 {
                cfg.warn("header rewrite", line, "missing header name/value");
                return;
            }
            let name = tokens[2].to_string();
            let value = Some(tokens[3..].join(" "));
            cfg.rewrites.push(RewriteRule {
                pattern,
                kind: RewriteKind::HeaderRewrite {
                    phase: Phase::Request,
                    name,
                    value,
                },
            });
        }
        "header-del" => {
            let name = tokens[2].to_string();
            cfg.rewrites.push(RewriteRule {
                pattern,
                kind: RewriteKind::HeaderRewrite {
                    phase: Phase::Request,
                    name,
                    value: None,
                },
            });
        }
        other => cfg.warn(
            "header rewrite",
            line,
            &format!("unrecognized header action '{other}'"),
        ),
    }
}

/// Surge / Loon `[Map Local]` 行解析：`pattern data="..." data-type=text status-code=200` → `Mock`。
///
/// pp-mitm `Mock` 无 headers 字段，`data-type` / `mime-type` 无法表达，记偏差。
fn parse_surge_map_local(cfg: &mut ImportedConfig, line: &str) {
    let tokens = split_tokens_keep_quoted(line);
    if tokens.len() < 2 {
        cfg.warn("map local", line, "unrecognized line");
        return;
    }
    let Some(pattern) = compile_pattern(&tokens[0], cfg, "map local", line) else {
        return;
    };
    let mut data: Option<String> = None;
    let mut status = 200u16;
    for pair in &tokens[1..] {
        let Some(eq) = pair.find('=') else {
            cfg.warn("map local", line, &format!("unrecognized token '{pair}'"));
            continue;
        };
        let (key, value) = (pair[..eq].trim(), pair[eq + 1..].trim());
        match key {
            "data" => data = Some(strip_quotes(value).to_string()),
            "status-code" => {
                if let Ok(code) = value.parse::<u16>() {
                    status = code;
                } else {
                    cfg.warn("map local", line, &format!("invalid status-code '{value}'"));
                }
            }
            "data-type" | "mime-type" => {
                cfg.warn(
                    "map local",
                    line,
                    &format!("{key} cannot be expressed (Mock has no headers), ignored"),
                );
            }
            other => cfg.warn(
                "map local",
                line,
                &format!("unrecognized parameter '{other}'"),
            ),
        }
    }
    let Some(body) = data else {
        cfg.warn("map local", line, "missing 'data' parameter");
        return;
    };
    cfg.rewrites.push(RewriteRule {
        pattern,
        kind: RewriteKind::Mock { status, body },
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use pp_mitm::RewriteKind;

    #[test]
    fn qx_rewrite_local_all_rule_types() {
        let content = r#"
# 注释
[rewrite_local]
^https?://example\.com/api/(.*) url-and-header https://cdn.example.com/api/$1
^https?://example\.com/redir url-307 https://target.example.com/
^https?://example\.com/old url-302 https://new.example.com/$1
^https?://example\.com/rsp script-response-body https://example.com/rsp.js
^https?://example\.com/req script-request-body https://example.com/req.js
^https?://example\.com/echo script-echo-response https://example.com/echo.js
^https?://example\.com/block reject
^https?://example\.com/block2 reject-200
^https?://example\.com/block3 reject-dict
^https?://example\.com/page url-response-body secret REDACTED
^https?://example\.com/hdr url-request-header token MASKED
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        // 8 条 rewrite：UrlRewrite x3、Reject x3、BodyRewrite x2
        assert_eq!(cfg.rewrites.len(), 8);
        assert!(matches!(
            cfg.rewrites[0].kind,
            RewriteKind::UrlRewrite { .. }
        ));
        assert!(matches!(
            cfg.rewrites[1].kind,
            RewriteKind::UrlRewrite { .. }
        ));
        assert!(matches!(
            cfg.rewrites[2].kind,
            RewriteKind::UrlRewrite { .. }
        ));
        assert!(matches!(cfg.rewrites[3].kind, RewriteKind::Reject));
        assert!(matches!(cfg.rewrites[4].kind, RewriteKind::Reject));
        assert!(matches!(cfg.rewrites[5].kind, RewriteKind::Reject));
        assert!(matches!(
            cfg.rewrites[6].kind,
            RewriteKind::BodyRewrite {
                phase: Phase::Response,
                ..
            }
        ));
        assert!(matches!(
            cfg.rewrites[7].kind,
            RewriteKind::BodyRewrite {
                phase: Phase::Request,
                ..
            }
        ));
        match &cfg.rewrites[0].kind {
            RewriteKind::UrlRewrite { target } => {
                assert_eq!(target, "https://cdn.example.com/api/$1")
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        match &cfg.rewrites[7].kind {
            RewriteKind::BodyRewrite { replacement, .. } => assert_eq!(replacement, "MASKED"),
            other => panic!("unexpected kind: {other:?}"),
        }

        // 3 条脚本钩子
        assert_eq!(cfg.scripts.len(), 3);
        assert_eq!(cfg.script_urls.len(), 3);
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(cfg.scripts[0].max_size, 131072);
        assert!(cfg.scripts[0].source.is_empty());
        assert_eq!(cfg.script_urls[0].0, "hook-0");
        assert_eq!(cfg.script_urls[0].1, "https://example.com/rsp.js");
        assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpRequest);
        assert!(cfg.scripts[1].requires_body);
        assert_eq!(cfg.script_urls[1].1, "https://example.com/req.js");
        assert_eq!(cfg.scripts[2].kind, ScriptKind::HttpResponse);
        assert!(!cfg.scripts[2].requires_body);
        assert_eq!(cfg.script_urls[2].1, "https://example.com/echo.js");

        // 302/307 与 BodyRewrite 偏差有记录
        assert!(!cfg.warnings.is_empty());
        assert!(cfg.warnings.iter().any(|w| w.contains("url-307")));
    }

    #[test]
    fn qx_task_local_and_mitm_hostnames() {
        let content = r#"
[task_local]
0 9 * * * https://example.com/sign.js, tag=每日签到, img-url=https://example.com/icon.png
30 12 * * * https://example.com/clean.js

[mitm]
hostname = *.example.com, api.example2.com, -exclude.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        assert_eq!(cfg.task_scripts.len(), 2);
        let (task, url) = &cfg.task_scripts[0];
        assert_eq!(task.name, "每日签到");
        assert_eq!(task.cron_expr, "0 0 9 * * *");
        assert!(task.source.is_empty());
        assert!(task.enabled);
        assert_eq!(task.dialect, ScriptDialect::QuantumultX);
        assert_eq!(url, "https://example.com/sign.js");

        let (task2, url2) = &cfg.task_scripts[1];
        assert_eq!(task2.name, "clean"); // 无 tag 时从 URL 派生
        assert_eq!(task2.cron_expr, "0 30 12 * * *");
        assert_eq!(url2, "https://example.com/clean.js");

        // `-` 前缀排除项记 warning，不进白名单
        assert_eq!(
            cfg.hostnames,
            vec!["*.example.com".to_string(), "api.example2.com".to_string()]
        );
        assert!(
            cfg.warnings
                .iter()
                .any(|w| w.contains("exclude.example.com"))
        );
    }

    #[test]
    fn surge_script_url_rewrite_header_rewrite_map_local_mitm() {
        let content = r#"
[Script]
json = type=http-response,pattern=^https://api\.example\.com/,script-path=https://example.com/json.js,requires-body=true,max-size=131072,timeout=10
req = type=http-request,pattern=^https://example\.com/,script-path=https://example.com/req.js
task = type=cron,cronexp="* * * * *",script-path=https://example.com/task.js

[URL Rewrite]
^https://old\.example\.com/ https://new.example.com/$1
^https://old2\.example\.com/ https://new2.example.com/ request-header

[Header Rewrite]
^https://example\.com/ header-replace X-Foo bar baz
^https://example\.com/ header-del X-Bar

[Map Local]
^https://example\.com/offline data="<h1>offline</h1>" data-type=text status-code=200

[MITM]
hostname = %APPEND% *.example.com, -exclude.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // [Script] 脚本钩子
        assert_eq!(cfg.scripts.len(), 2);
        assert_eq!(cfg.script_urls.len(), 2);
        assert_eq!(cfg.scripts[0].name, "json");
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(cfg.scripts[0].max_size, 131072);
        assert_eq!(
            cfg.scripts[0].pattern.as_str(),
            "^https://api\\.example\\.com/"
        );
        assert!(cfg.scripts[0].source.is_empty());
        assert_eq!(
            cfg.script_urls[0],
            (
                "json".to_string(),
                "https://example.com/json.js".to_string()
            )
        );
        assert_eq!(cfg.scripts[1].kind, ScriptKind::HttpRequest);
        assert!(!cfg.scripts[1].requires_body);
        assert_eq!(cfg.scripts[1].max_size, 131072); // 未指定 max-size 时的默认值

        // [Script] cron → TaskScript（5 段补成 6 段）
        assert_eq!(cfg.task_scripts.len(), 1);
        let (task, url) = &cfg.task_scripts[0];
        assert_eq!(task.name, "task");
        assert_eq!(task.cron_expr, "0 * * * * *");
        assert_eq!(task.dialect, ScriptDialect::Surge);
        assert!(task.enabled);
        assert!(task.source.is_empty());
        assert_eq!(url, "https://example.com/task.js");

        // [URL Rewrite] 2 条
        match &cfg.rewrites[0].kind {
            RewriteKind::UrlRewrite { target } => {
                assert_eq!(target, "https://new.example.com/$1")
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        assert!(matches!(
            cfg.rewrites[1].kind,
            RewriteKind::UrlRewrite { .. }
        ));

        // [Header Rewrite] 2 条
        match &cfg.rewrites[2].kind {
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name,
                value,
            } => {
                assert_eq!(name, "X-Foo");
                assert_eq!(value.as_deref(), Some("bar baz"));
            }
            other => panic!("unexpected kind: {other:?}"),
        }
        match &cfg.rewrites[3].kind {
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name,
                value,
            } => {
                assert_eq!(name, "X-Bar");
                assert_eq!(value, &None);
            }
            other => panic!("unexpected kind: {other:?}"),
        }

        // [Map Local] → Mock
        match &cfg.rewrites[4].kind {
            RewriteKind::Mock { status, body } => {
                assert_eq!(*status, 200);
                assert_eq!(body, "<h1>offline</h1>");
            }
            other => panic!("unexpected kind: {other:?}"),
        }

        // [MITM]：%APPEND% 剥离，- 排除记 warning
        assert_eq!(cfg.hostnames, vec!["*.example.com".to_string()]);
        assert!(
            cfg.warnings
                .iter()
                .any(|w| w.contains("exclude.example.com"))
        );
        assert!(cfg.warnings.iter().any(|w| w.contains("request-header")));
    }

    #[test]
    fn skips_unknown_comment_blank_lines_with_warnings() {
        let content = r#"
[rewrite_local]
# 注释
; 也是注释

^https?://example\.com/ok url-and-header https://ok.example.com/
totally unknown line
also unknown
[task_local]
garbage
[mitm]
hostname = *.example.com
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();

        assert_eq!(cfg.rewrites.len(), 1);
        assert_eq!(cfg.hostnames, vec!["*.example.com".to_string()]);
        assert_eq!(cfg.task_scripts.len(), 0);
        // 未知行与无法解析的任务行计入 warnings；注释/空行静默跳过
        assert_eq!(cfg.warnings.len(), 3);
    }

    #[test]
    fn invalid_regex_skipped_with_warning() {
        let content = r#"
[rewrite_local]
( url-and-header https://broken.example.com/
^https?://ok\.example\.com/ url-and-header https://ok2.example.com/

[Script]
bad = type=http-response,pattern=(,script-path=https://example.com/bad.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // 非法正则在 rewrite 与 script 中各记 1 条 warning，不 panic
        assert_eq!(cfg.rewrites.len(), 1);
        assert_eq!(cfg.scripts.len(), 0);
        assert_eq!(
            cfg.warnings
                .iter()
                .filter(|w| w.contains("invalid regex"))
                .count(),
            2
        );
    }

    #[test]
    fn parse_config_meta_extracts_header_fields_and_stops_at_section() {
        let content = r#"#!name=扫描全能王-解锁VIP
#!desc=扫描全能王-手机扫描仪 解锁黄金会员
#!date=2026-01-21
#!category=🐹 BOBO Premium
#!author=叮当猫chxm1023[https://github.com/chxm1023/Rewrite]
#!icon=https://example.com/CamScanner.png
#!openUrl=https://apps.apple.com/app/id388627783

[Script]
rule = type=http-response,pattern=^https://api-cs\.intsig\.net/,script-path=https://example.com/camscanner.js
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta.name.as_deref(), Some("扫描全能王-解锁VIP"));
        assert_eq!(
            meta.desc.as_deref(),
            Some("扫描全能王-手机扫描仪 解锁黄金会员")
        );
        assert_eq!(meta.date.as_deref(), Some("2026-01-21"));
        assert_eq!(meta.category.as_deref(), Some("🐹 BOBO Premium"));
        assert_eq!(
            meta.author.as_deref(),
            Some("叮当猫chxm1023[https://github.com/chxm1023/Rewrite]")
        );
        assert_eq!(
            meta.icon.as_deref(),
            Some("https://example.com/CamScanner.png")
        );
        assert_eq!(
            meta.open_url.as_deref(),
            Some("https://apps.apple.com/app/id388627783")
        );

        // parse_import 同步回填 meta
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.meta.name.as_deref(), Some("扫描全能王-解锁VIP"));
        assert_eq!(
            cfg.meta.open_url.as_deref(),
            Some("https://apps.apple.com/app/id388627783")
        );
    }

    #[test]
    fn parse_config_meta_returns_all_none_without_header() {
        let content = r#"[rewrite_local]
^https?://example\.com/ url-and-header https://target.example.com/
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta, ConfigMeta::default());
        assert!(meta.name.is_none());
        assert!(meta.desc.is_none());
        assert!(meta.author.is_none());
        assert!(meta.icon.is_none());
        assert!(meta.date.is_none());
        assert!(meta.category.is_none());
        assert!(meta.open_url.is_none());
    }

    #[test]
    fn parse_config_meta_stops_at_first_non_bang_line() {
        // `#` 注释（非 `#!`）立即中止头部解析
        let content = "#!name=有效名称\n# 普通注释\n#!desc=不应被解析\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.name.as_deref(), Some("有效名称"));
        assert!(meta.desc.is_none());
        // 空行同样中止
        let content2 = "#!name=A\n\n#!desc=B\n";
        let meta2 = parse_config_meta(content2);
        assert_eq!(meta2.name.as_deref(), Some("A"));
        assert!(meta2.desc.is_none());
    }

    /// `#!description=` 是 `#!desc=` 的别名：单键即可解析为 `desc`。
    #[test]
    fn parse_config_meta_accepts_description_alias() {
        let content = "#!name=Demo\n#!description=使用说明\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.desc.as_deref(), Some("使用说明"));

        // 经 parse_import 同步回填一致（嗅探/导入/缓存共用 parse_config_meta）。
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.meta.desc.as_deref(), Some("使用说明"));
    }

    /// `#!desc=` 与 `#!description=` 并存时 `#!desc=` 优先，别名不覆盖。
    #[test]
    fn parse_config_meta_description_alias_does_not_override_desc() {
        // desc 在前、description 在后：别名不覆盖。
        let content = "#!name=Demo\n#!desc=短描述\n#!description=全拼描述\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.desc.as_deref(), Some("短描述"));

        // description 在前、desc 在后：规范键仍生效（desc 优先）。
        let content2 = "#!name=Demo\n#!description=全拼描述\n#!desc=短描述\n";
        let meta2 = parse_config_meta(content2);
        assert_eq!(meta2.desc.as_deref(), Some("短描述"));
    }

    /// BaiDuTieBa 样例回归：`#!desc=` 值含分号与中文，别名改动不干扰规范键解析。
    #[test]
    fn parse_config_meta_baidu_tieba_desc_semicolon_chinese() {
        let content = "#!name=百度贴吧\n#!desc=开屏广告;推荐和吧内帖子列表的直播及广告\n";
        let meta = parse_config_meta(content);
        assert_eq!(
            meta.desc.as_deref(),
            Some("开屏广告;推荐和吧内帖子列表的直播及广告")
        );

        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(
            cfg.meta.desc.as_deref(),
            Some("开屏广告;推荐和吧内帖子列表的直播及广告")
        );
    }

    /// ① `#!arguments=` / `#!arguments-desc=` 解析：键/默认值/描述合并进 ArgSpec。
    #[test]
    fn config_meta_parses_arguments_decl_and_strict_desc() {
        let content = r#"#!name=Demo
#!arguments= server:api.example.com, token:default-token
#!arguments-desc= {"server":"API 服务器", "token":"鉴权令牌"}
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta.arguments.len(), 2);
        let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
        assert_eq!(server.default_value, "api.example.com");
        assert_eq!(server.description.as_deref(), Some("API 服务器"));
        let token = meta.arguments.iter().find(|a| a.key == "token").unwrap();
        assert_eq!(token.default_value, "default-token");
        assert_eq!(token.description.as_deref(), Some("鉴权令牌"));
        // 默认值含空格：值整体保留
        let content2 = "#!arguments= server:api.example.com, greeting:hello world\n";
        let meta2 = parse_config_meta(content2);
        assert_eq!(
            meta2
                .arguments
                .iter()
                .find(|a| a.key == "greeting")
                .unwrap()
                .default_value,
            "hello world"
        );
    }

    /// ① 非严格 JSON 的 desc 语法（键未加引号）走宽松提取；声明缺失的键补空默认值。
    #[test]
    fn config_meta_parses_arguments_desc_loosely() {
        let content = "#!arguments= server:api.example.com\n#!arguments-desc= {server:\"API 服务器\", token: '鉴权 令牌'}\n";
        let meta = parse_config_meta(content);
        let server = meta.arguments.iter().find(|a| a.key == "server").unwrap();
        assert_eq!(server.default_value, "api.example.com");
        assert_eq!(server.description.as_deref(), Some("API 服务器"));
        // 仅出现在 desc 中的键：默认值空串补 ArgSpec。
        let token = meta.arguments.iter().find(|a| a.key == "token").unwrap();
        assert_eq!(token.default_value, "");
        assert_eq!(token.description.as_deref(), Some("鉴权 令牌"));
    }

    /// ② `[Script]` 行 `argument=` 参数提取进 ScriptRule.argument。
    #[test]
    fn surge_script_line_extracts_argument_into_script_rule() {
        let content = r#"#!name=Demo
#!arguments= server:api.example.com, token:default-token
[Script]
xxx = type=http-response, pattern=^https://api\.example\.com/, script-path=https://example.com/json.js, argument={server}|{token}
plain = type=http-request, pattern=^https://example\.com/, script-path=https://example.com/req.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.scripts.len(), 2);
        assert_eq!(cfg.scripts[0].argument.as_deref(), Some("{server}|{token}"));
        // 未声明 argument 的脚本保持 None。
        assert_eq!(cfg.scripts[1].argument, None);
        // meta 同步解析出 arguments。
        assert_eq!(cfg.meta.arguments.len(), 2);
    }

    /// ① `#!arguments-desc=` 朴素语法 `key:描述`（无 `{}`、无引号，可含中文/空格，
    /// 逗号分隔多个）→ ArgSpec.description 正确填充。
    #[test]
    fn config_meta_parses_arguments_desc_naive_syntax() {
        // 单条朴素语法（BaiDuTieBa 真实样例）。
        let content = r#"#!arguments=per_filter_video:0
#!arguments-desc=per_filter_video:设置为1则推荐页不展示视频贴
"#;
        let meta = parse_config_meta(content);
        assert_eq!(meta.arguments.len(), 1);
        let spec = &meta.arguments[0];
        assert_eq!(spec.key, "per_filter_video");
        assert_eq!(spec.default_value, "0");
        assert_eq!(
            spec.description.as_deref(),
            Some("设置为1则推荐页不展示视频贴")
        );

        // 多条逗号分隔：描述内的逗号不误切，仅 `,key:` 处切分。
        let content2 = r#"#!arguments=per_filter_video:0, banner:1
#!arguments-desc=per_filter_video:推荐页关闭视频贴, banner:顶部横幅开关
"#;
        let meta2 = parse_config_meta(content2);
        assert_eq!(meta2.arguments.len(), 2);
        let pf = meta2
            .arguments
            .iter()
            .find(|a| a.key == "per_filter_video")
            .unwrap();
        assert_eq!(pf.default_value, "0");
        assert_eq!(pf.description.as_deref(), Some("推荐页关闭视频贴"));
        let banner = meta2.arguments.iter().find(|a| a.key == "banner").unwrap();
        assert_eq!(banner.default_value, "1");
        assert_eq!(banner.description.as_deref(), Some("顶部横幅开关"));

        // 描述含逗号（非 `key:` 前缀）不切分。
        let content3 = "#!arguments-desc=per_filter_video:推荐页关闭视频,弹窗\n";
        let meta3 = parse_config_meta(content3);
        assert_eq!(
            meta3.arguments[0].description.as_deref(),
            Some("推荐页关闭视频,弹窗")
        );
    }

    /// ③ Surge `[Script]` 行 `requires-body` 数字形式 `1/0` 与布尔 `true/false` 均正确解析。
    #[test]
    fn surge_script_requires_body_accepts_numeric_and_boolean() {
        let content = r#"
[Script]
num1 = type=http-response,pattern=^https://a\.com/,script-path=https://example.com/a.js,requires-body=1
num0 = type=http-response,pattern=^https://b\.com/,script-path=https://example.com/b.js,requires-body=0
bt = type=http-response,pattern=^https://c\.com/,script-path=https://example.com/c.js,requires-body=true
bf = type=http-response,pattern=^https://d\.com/,script-path=https://example.com/d.js,requires-body=false
none = type=http-response,pattern=^https://e\.com/,script-path=https://example.com/e.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.scripts.len(), 5);
        assert!(cfg.scripts[0].requires_body, "requires-body=1 → true");
        assert!(!cfg.scripts[1].requires_body, "requires-body=0 → false");
        assert!(cfg.scripts[2].requires_body, "requires-body=true → true");
        assert!(!cfg.scripts[3].requires_body, "requires-body=false → false");
        assert!(!cfg.scripts[4].requires_body, "缺省 → false");
    }

    /// ④ Surge `max-size=-1` / `0`（unlimited）映射为 10MB 上限；正常数字原样解析；
    /// 缺省回退 131072。
    #[test]
    fn surge_script_max_size_unlimited_and_normal_values() {
        let content = r#"
[Script]
unlimited = type=http-response,pattern=^https://a\.com/,script-path=https://example.com/a.js,max-size=-1
zero = type=http-response,pattern=^https://b\.com/,script-path=https://example.com/b.js,max-size=0
normal = type=http-response,pattern=^https://c\.com/,script-path=https://example.com/c.js,max-size=4096
default = type=http-response,pattern=^https://d\.com/,script-path=https://example.com/d.js
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();
        assert_eq!(cfg.scripts.len(), 4);
        assert_eq!(
            cfg.scripts[0].max_size,
            10 * 1024 * 1024,
            "max-size=-1 → unlimited"
        );
        assert_eq!(
            cfg.scripts[1].max_size,
            10 * 1024 * 1024,
            "max-size=0 → unlimited"
        );
        assert_eq!(cfg.scripts[2].max_size, 4096, "正常数字原样");
        assert_eq!(cfg.scripts[3].max_size, 131072, "缺省回退");
    }

    /// ①③④ BaiDuTieBa.sgmodule 真实样例整段断言：朴素 desc、三花括号占位原样保留、
    /// `requires-body=1`、`max-size=-1` 与 pattern 中 `\/(...)` 均正确解析。
    #[test]
    fn surge_script_badubatieba_sample_fixture() {
        let content = r#"#!arguments=per_filter_video:0
#!arguments-desc=per_filter_video:设置为1则推荐页不展示视频贴
[Script]
贴吧proto = type=http-response,pattern=^https?:\/\/(tiebac|c\.tieba)\.baidu\.com\/...$ ,requires-body=1,binary-body-mode=1,max-size=-1,script-path=https://example.com/x.js,argument=per_filter_video_thread={{{per_filter_video}}}
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // 头部参数：朴素 desc 合并进 ArgSpec。
        assert_eq!(cfg.meta.arguments.len(), 1);
        let spec = &cfg.meta.arguments[0];
        assert_eq!(spec.key, "per_filter_video");
        assert_eq!(spec.default_value, "0");
        assert_eq!(
            spec.description.as_deref(),
            Some("设置为1则推荐页不展示视频贴")
        );

        // [Script] 行：requires-body=1 / max-size=-1 / argument 三花括号占位保留原文。
        assert_eq!(cfg.scripts.len(), 1);
        assert_eq!(cfg.scripts[0].name, "贴吧proto");
        assert_eq!(
            cfg.scripts[0].pattern.as_str(),
            r"^https?:\/\/(tiebac|c\.tieba)\.baidu\.com\/...$"
        );
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(cfg.scripts[0].max_size, 10 * 1024 * 1024);
        assert_eq!(
            cfg.scripts[0].argument.as_deref(),
            Some("per_filter_video_thread={{{per_filter_video}}}")
        );
        assert_eq!(cfg.script_urls[0].1, "https://example.com/x.js");
    }

    /// ② QX rewrite 脚本行同样提取 `argument=`。
    #[test]
    fn qx_rewrite_script_line_extracts_argument() {
        let content = r#"
[rewrite_local]
^https?://api\.example\.com/(.*) script-response-body https://example.com/rsp.js argument={server}
"#;
        let cfg = parse_import(content, ScriptDialect::QuantumultX).unwrap();
        assert_eq!(cfg.scripts.len(), 1);
        assert_eq!(cfg.scripts[0].argument.as_deref(), Some("{server}"));
    }

    /// ① `#!` 头 `=` 两侧空格：key trim 后匹配、value trim。
    #[test]
    fn parse_config_meta_trims_whitespace_around_equals() {
        let content = "#!name = 测试名称 \n#!desc = 描述内容\n#!author =  作者  \n#!icon = https://example.com/i.png\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.name.as_deref(), Some("测试名称"));
        assert_eq!(meta.desc.as_deref(), Some("描述内容"));
        assert_eq!(meta.author.as_deref(), Some("作者"));
        assert_eq!(meta.icon.as_deref(), Some("https://example.com/i.png"));
    }

    /// ② `#!arguments` 引号感知切分：`Types:"Translate,External"` 不切碎、值去引号、
    /// key 含 `[0]` 下标正常。
    #[test]
    fn parse_arguments_decl_quote_aware_split_and_unquote() {
        let content = "#!arguments = Types:\"Translate,External\",Languages[0]:\"AUTO\",Vendor:\"Google\",plain:value\n";
        let meta = parse_config_meta(content);
        assert_eq!(meta.arguments.len(), 4);
        let types = meta.arguments.iter().find(|a| a.key == "Types").unwrap();
        assert_eq!(types.default_value, "Translate,External");
        let lang = meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[0]")
            .unwrap();
        assert_eq!(lang.default_value, "AUTO");
        let vendor = meta.arguments.iter().find(|a| a.key == "Vendor").unwrap();
        assert_eq!(vendor.default_value, "Google");
        // 无引号裸值原样解析（回归旧语法）。
        let plain = meta.arguments.iter().find(|a| a.key == "plain").unwrap();
        assert_eq!(plain.default_value, "value");
    }

    /// ①② Surge `.sgmodule` 样例：`#!arguments` 引号感知切分 + Surge `[Script]` 行
    /// 的 `{{{key}}}` 占位原文与 `engine` 忽略。
    #[test]
    fn dualsubs_spotify_surge_sgmodule_sample_fixture() {
        let content = r#"#!name = 🍿️ DualSubs: 🎵 Spotify
#!desc = Spotify 增强及双语歌词
#!arguments = Types:"Translate,External",Languages[0]:"AUTO",Languages[1]:"ZH",Vendor:"Google",LogLevel:"WARN"
[Script]
🍿️ DualSubs.Spotify.Tracks = type=http-response, pattern=^https?:\/\/api\.spotify\.com\/v1\/tracks\?, requires-body=1, engine=webview, script-path=https://example.com/r.js, argument=Types="{{{Types}}}"&Languages[0]="{{{Languages[0]}}}"&Vendor="{{{Vendor}}}"
"#;
        let cfg = parse_import(content, ScriptDialect::Surge).unwrap();

        // ① 头空格 trim。
        assert_eq!(cfg.meta.name.as_deref(), Some("🍿️ DualSubs: 🎵 Spotify"));
        assert_eq!(cfg.meta.desc.as_deref(), Some("Spotify 增强及双语歌词"));

        // ② `#!arguments` 引号感知：含逗号值不切碎、key 含 `[0]`、去引号。
        assert_eq!(cfg.meta.arguments.len(), 5);
        let types = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Types")
            .unwrap();
        assert_eq!(types.default_value, "Translate,External");
        let lang0 = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[0]")
            .unwrap();
        assert_eq!(lang0.default_value, "AUTO");
        let lang1 = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Languages[1]")
            .unwrap();
        assert_eq!(lang1.default_value, "ZH");
        let vendor = cfg
            .meta
            .arguments
            .iter()
            .find(|a| a.key == "Vendor")
            .unwrap();
        assert_eq!(vendor.default_value, "Google");

        // Surge [Script]：name = type=...；`{{{key}}}` 占位原文保留，engine 忽略。
        assert_eq!(cfg.scripts.len(), 1);
        assert_eq!(cfg.scripts[0].name, "🍿️ DualSubs.Spotify.Tracks");
        assert_eq!(cfg.scripts[0].kind, ScriptKind::HttpResponse);
        assert!(cfg.scripts[0].requires_body);
        assert_eq!(
            cfg.scripts[0].argument.as_deref(),
            Some(
                "Types=\"{{{Types}}}\"&Languages[0]=\"{{{Languages[0]}}}\"&Vendor=\"{{{Vendor}}}\""
            )
        );
        assert_eq!(cfg.script_urls[0].1, "https://example.com/r.js");
    }
}
