//! Quantumult X dialect parsing (`[rewrite_local]`, `[rewrite_remote]`, `[task_local]`).
//!
//! QX 生态配置常混入其他方言行（Loon 风格 `http-request ...`、Surge 风格
//! `name = type=...`），[`parse_qx_rewrite`] 先按行型识别分派，再按 QX 语法解析。

use pp_mitm::{Phase, RewriteKind};
use pp_script::{ScriptDialect, ScriptKind, TaskScript};

use super::loon::parse_loon_script;
use super::surge::parse_surge_script;
use super::utils::*;

/// QX `[rewrite_local]` / `[rewrite_remote]` line parsing.
///
/// 支持的 action（`url` 修饰符可省略）：
///
/// | action | Internal Rule |
/// |--------|---------------|
/// | `url-and-header target` | `UrlRewrite`（header 部分丢弃，记偏差） |
/// | `url-302` / `url-307` / `302` / `307 target` | `Redirect` |
/// | `reject` / `reject-200` / `reject-img` / `reject-dict` / `reject-array` | `Reject` 各变体 |
/// | `script-request-header` / `script-request-body` / `script-response-header` / `script-response-body` | `ScriptRule`（requires-body 按名） |
/// | `script-echo-response` | `ScriptRule{HttpRequest, no body}`（请求阶段合成响应） |
/// | `script-analyze-echo-response` | `ScriptRule{HttpRequest, requires body}`（含完整 $request） |
/// | `echo-response <ctype> echo-response <body>` | `Mock` |
/// | `[url-]request-header <regex> [request-header] <repl>` | `HeaderBlockRewrite{Request}` |
/// | `[url-]response-header <regex> [response-header] <repl>` | `HeaderBlockRewrite{Response}` |
/// | `[url-]request-body <regex> [request-body] <repl>` | `BodyRewrite{Request, body_pattern}` |
/// | `[url-]response-body <regex> [response-body] <repl>` | `BodyRewrite{Response, body_pattern}` |
///
/// `jsonjq-*`（jq 语义）无引擎可表达，记 warning 跳过。
/// `[rewrite_remote]` 远程引用行（`url, tag=...`）无法内联解析，记入 warnings。
pub(super) fn parse_qx_rewrite(
    cfg: &mut super::ImportedConfig,
    hook_index: &mut usize,
    dialect: ScriptDialect,
    line: &str,
) {
    // 混入的其他方言行：Loon 风格以 `http-request|http-response` 起始；
    // Surge 风格为 `name = type=...`（QX rewrite 行的 pattern 是正则，通常不含 `=`）。
    let first = line.split_whitespace().next().unwrap_or_default();
    if matches!(first, "http-request" | "http-response") {
        parse_loon_script(cfg, line);
        return;
    }
    if looks_like_surge_kv(line) {
        parse_surge_script(cfg, dialect, line);
        return;
    }

    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        cfg.warn("rewrite", line, "unrecognized line");
        return;
    }
    // 兼容生态写法 `pattern url <action> ...`（`url` 修饰符可省略）。
    let (pattern_src, action, args) = if tokens.len() >= 3 && tokens[1] == "url" {
        (tokens[0], tokens[2], &tokens[3..])
    } else {
        (tokens[0], tokens[1], &tokens[2..])
    };
    let Some(pattern) = compile_pattern(pattern_src, cfg, "rewrite", line) else {
        return;
    };
    match action {
        "url-and-header" => {
            let Some(target) = args.first() else {
                cfg.warn("rewrite", line, "missing rewrite target");
                return;
            };
            cfg.warn("rewrite", line, "url-and-header header part discarded");
            cfg.rewrites.push(pp_mitm::RewriteRule {
                pattern,
                kind: RewriteKind::UrlRewrite {
                    target: (*target).to_string(),
                },
            });
        }
        "url-302" | "url-307" | "302" | "307" => {
            let Some(target) = args.first() else {
                cfg.warn("rewrite", line, "missing redirect target");
                return;
            };
            let status = if action.ends_with("307") { 307 } else { 302 };
            cfg.rewrites.push(pp_mitm::RewriteRule {
                pattern,
                kind: RewriteKind::Redirect {
                    status,
                    target: (*target).to_string(),
                },
            });
        }
        "reject" | "reject-200" | "reject-img" | "reject-dict" | "reject-array" | "reject-json" => {
            let kind = match action {
                "reject-200" => RewriteKind::reject_200(),
                "reject-img" => RewriteKind::reject_img(),
                "reject-dict" | "reject-json" => RewriteKind::reject_json("{}"),
                "reject-array" => RewriteKind::reject_json("[]"),
                _ => RewriteKind::reject(),
            };
            cfg.rewrites.push(pp_mitm::RewriteRule { pattern, kind });
        }
        "script-request-header"
        | "script-request-body"
        | "script-response-header"
        | "script-response-body"
        | "script-echo-response"
        | "script-analyze-echo-response" => {
            parse_qx_script_hook(cfg, hook_index, line, pattern, action, args);
        }
        "jsonjq-request-body" | "jsonjq-response-body" => {
            cfg.warn("rewrite", line, "jsonjq (jq syntax) not supported, skipped");
        }
        "echo-response" => {
            // `pattern url echo-response <content-type> echo-response <body...>`
            let Some(content_type) = args.first() else {
                cfg.warn("rewrite", line, "missing echo-response content-type");
                return;
            };
            let body_tokens = if args.get(1) == Some(&"echo-response") {
                &args[2..]
            } else {
                &args[1..]
            };
            if body_tokens.is_empty() {
                cfg.warn("rewrite", line, "missing echo-response body");
                return;
            }
            cfg.rewrites.push(pp_mitm::RewriteRule {
                pattern,
                kind: RewriteKind::Mock {
                    status: 200,
                    body: body_tokens.join(" ").into_bytes(),
                    headers: vec![("Content-Type".to_string(), (*content_type).to_string())],
                },
            });
        }
        "request-header"
        | "response-header"
        | "request-body"
        | "response-body"
        | "url-request-header"
        | "url-response-header"
        | "url-request-body"
        | "url-response-body" => {
            parse_qx_regex_rewrite(cfg, line, pattern, action, args);
        }
        other => cfg.warn("rewrite", line, &format!("unrecognized action '{other}'")),
    }
}

/// QX script hook lines: `pattern [url] <action> <script-path> [argument=...]`.
fn parse_qx_script_hook(
    cfg: &mut super::ImportedConfig,
    hook_index: &mut usize,
    line: &str,
    pattern: regex::Regex,
    action: &str,
    args: &[&str],
) {
    let Some(path) = args.first() else {
        cfg.warn("rewrite", line, "missing script path");
        return;
    };
    let (kind, requires_body) = match action {
        "script-request-header" => (ScriptKind::HttpRequest, false),
        "script-request-body" => (ScriptKind::HttpRequest, true),
        "script-response-header" => (ScriptKind::HttpResponse, false),
        // echo-response 在请求阶段由脚本直接合成响应（不经网络）；
        // analyze 变体向脚本提供含 body 的完整 $request。
        "script-echo-response" => (ScriptKind::HttpRequest, false),
        "script-analyze-echo-response" => (ScriptKind::HttpRequest, true),
        _ => (ScriptKind::HttpResponse, true),
    };
    let name = format!("hook-{hook_index}");
    *hook_index += 1;
    if is_remote_url(path) {
        cfg.script_urls.push((name.clone(), (*path).to_string()));
    } else {
        cfg.warn("rewrite", line, "local script path not supported, skipped");
    }
    // QX script lines also support `argument={key}` additional parameters
    // (space-separated trailing argument=).
    let argument = args
        .get(1..)
        .map(|rest| parse_kv_params(&rest.join(" ")))
        .and_then(|params| params.get("argument").cloned());
    cfg.scripts.push(pp_mitm::ScriptRule {
        name,
        kind,
        pattern,
        requires_body,
        max_size: 131072,
        source: String::new(),
        argument,
    });
}

/// QX regex rewrite family: `[url-]<request|response>-<header|body> <regex> [<keyword>] <repl>`.
///
/// 双关键词形态（`pattern url response-body <regex> response-body <repl>`）以第二个关键词
/// token 分隔 body 正则与替换串；单关键词形态（`url-response-body <regex> <repl>`）与
/// 省略第二关键词的形态取 `args[0]` 为正则、其余 join 为替换串。
fn parse_qx_regex_rewrite(
    cfg: &mut super::ImportedConfig,
    line: &str,
    pattern: regex::Regex,
    action: &str,
    args: &[&str],
) {
    let Some((regex_src, replacement)) = split_regex_replacement(action, args) else {
        cfg.warn("rewrite", line, "missing body/header regex or replacement");
        return;
    };
    let Some(body_or_block_pattern) = compile_pattern(&regex_src, cfg, "rewrite", line) else {
        return;
    };
    let phase = if action.contains("request") {
        Phase::Request
    } else {
        Phase::Response
    };
    let kind = if action.contains("header") {
        RewriteKind::HeaderBlockRewrite {
            phase,
            block_pattern: body_or_block_pattern,
            replacement,
        }
    } else {
        RewriteKind::BodyRewrite {
            phase,
            body_pattern: Some(body_or_block_pattern),
            replacement,
        }
    };
    cfg.rewrites.push(pp_mitm::RewriteRule { pattern, kind });
}

/// Split QX regex rewrite args into `(regex, replacement)`.
///
/// `response-body <regex> response-body <repl>` 等双关键词形态按第二个关键词切分；
/// `url-response-body <regex> <repl>` 单关键词形态取首 token 为正则。
fn split_regex_replacement(action: &str, args: &[&str]) -> Option<(String, String)> {
    if !action.starts_with("url-")
        && let Some(idx) = args.iter().position(|t| *t == action)
        && idx >= 1
        && args.len() > idx + 1
    {
        return Some((args[..idx].join(" "), args[idx + 1..].join(" ")));
    }
    if args.len() >= 2 {
        return Some((args[0].to_string(), args[1..].join(" ")));
    }
    None
}

/// 判断是否为 Surge 风格 `name = type=...` 行（QX 文件混入场景）。
///
/// 等号左侧是脚本名（不是以 `^` 开头的正则，也不是 URL），且右侧参数含 `type=`。
fn looks_like_surge_kv(line: &str) -> bool {
    let Some(eq) = line.find('=') else {
        return false;
    };
    let key = line[..eq].trim();
    if key.is_empty() || key.starts_with('^') || key.contains("://") {
        return false;
    }
    parse_kv_params(&line[eq + 1..]).contains_key("type")
}

/// QX `[task_local]` line parsing: `<5-part cron> <script-url>[, tag=..., img-url=...]`.
///
/// pp-script's cron crate needs 6 parts, prefix with `"0 "` during parsing;
/// `name` takes `tag`, falls back to URL-derived name when missing.
pub(super) fn parse_qx_task(cfg: &mut super::ImportedConfig, dialect: ScriptDialect, line: &str) {
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
        if let Some((k, v)) = pair.trim().split_once('=')
            && k.trim().eq_ignore_ascii_case("tag")
        {
            tag = Some(strip_quotes(v.trim()).to_string());
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
