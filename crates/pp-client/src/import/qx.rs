//! Quantumult X dialect parsing (`[rewrite_local]`, `[rewrite_remote]`, `[task_local]`).

use pp_mitm::{Phase, RewriteKind};
use pp_script::{ScriptDialect, ScriptKind, TaskScript};

use super::utils::*;

/// QX `[rewrite_local]` / `[rewrite_remote]` line parsing.
///
/// | Input | Internal Rule | Deviation |
/// |------|----------|------|
/// | `pattern url-and-header target` | `UrlRewrite` | header part discarded |
/// | `pattern url-307/url-302 target` | `UrlRewrite` | redirect status code lost (recorded deviation) |
/// | `pattern script-response-body path` | `ScriptRule{HttpResponse}` | body limit fixed 131072 |
/// | `pattern script-request-body path` | `ScriptRule{HttpRequest}` | same as above |
/// | `pattern script-echo-response path` | `ScriptRule{HttpResponse, no body}` | same as above |
/// | `pattern reject/reject-200/reject-dict` | `Reject` | reject status code/content lost |
/// | `pattern url-response-body regex repl` | `BodyRewrite{Response}` | body regex lost (recorded deviation) |
/// | `pattern url-request-header regex repl` | `BodyRewrite{Request}` | semantic approximation (recorded deviation) |
///
/// `[rewrite_remote]` remote reference lines (`url, tag=...`) cannot be parsed inline,
/// recorded in warnings.
pub(super) fn parse_qx_rewrite(
    cfg: &mut super::ImportedConfig,
    hook_index: &mut usize,
    line: &str,
) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        cfg.warn("rewrite", line, "unrecognized line");
        return;
    }
    // Compatible with some ecosystem styles `pattern url script-response-body <path>`
    // (extra `url` modifier ignored)
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
            cfg.rewrites.push(pp_mitm::RewriteRule {
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
            // QX script lines also support `argument={key}` additional parameters
            // (like rewrite_remote's `..., argument=xxx` style; rewrite_local can also have
            // space-separated trailing argument=).
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
        "reject" | "reject-200" | "reject-dict" => {
            cfg.rewrites.push(pp_mitm::RewriteRule {
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
            cfg.rewrites.push(pp_mitm::RewriteRule {
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
