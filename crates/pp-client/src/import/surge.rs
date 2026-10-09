//! Surge dialect parsing (`[Script]`, `[URL Rewrite]`, `[Header Rewrite]`,
//! `[Body Rewrite]`, `[Map Local]`).

use pp_mitm::{Phase, RewriteKind};
use pp_script::{ScriptDialect, ScriptKind};

use super::utils::*;

/// Surge / Loon `[Script]` line parsing: `name = type=...,pattern=...,script-path=...`.
///
/// | type | Internal Rule | Description |
/// |------|----------|------|
/// | `http-response` | `ScriptRule{HttpResponse}` | default `requires-body=false`, `max-size=131072` |
/// | `http-request` | `ScriptRule{HttpRequest}` | same as above |
/// | `cron` | `TaskScript` | `cronexp` 5-part prefixed with `"0 "` to become 6-part |
///
/// Loon and Surge parameter name differences (`http-types` / `require-body`) are aliased.
pub(super) fn parse_surge_script(
    cfg: &mut super::ImportedConfig,
    dialect: ScriptDialect,
    line: &str,
) {
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
            // `max-size=-1` / `0` (Surge unlimited) mapped to 10MB upper bound, see [`parse_max_size`].
            let max_size = parse_max_size(params.get("max-size")).unwrap_or(131072);
            // Surge/Loon script line parameter `argument={key}|...` (template placeholder replaced at runtime).
            let argument = params.get("argument").cloned();
            cfg.script_urls
                .push((name.to_string(), script_path.clone()));
            cfg.scripts.push(pp_mitm::ScriptRule {
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
                pp_script::TaskScript {
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

/// Surge / Loon `[URL Rewrite]` line parsing.
///
/// - `pattern target` → `UrlRewrite`
/// - `pattern target 302|307` → `Redirect`（尾部 mode token，QX 兼容写法）
/// - `pattern - reject` → `Reject`（`-` 占位 target + reject mode）
/// - `pattern target header` → `UrlRewrite` + 偏差（request-header 参数无法表达）
pub(super) fn parse_surge_url_rewrite(cfg: &mut super::ImportedConfig, line: &str) {
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
    let kind = match tokens.last() {
        Some(&"reject") => RewriteKind::reject(),
        Some(mode @ (&"302" | &"307")) => RewriteKind::Redirect {
            status: if *mode == "307" { 307 } else { 302 },
            target: tokens[1].to_string(),
        },
        _ => {
            if tokens.len() > 2 {
                cfg.warn(
                    "url rewrite",
                    line,
                    "request-header argument cannot be expressed, only URL rewritten",
                );
            }
            RewriteKind::UrlRewrite {
                target: tokens[1].to_string(),
            }
        }
    };
    cfg.rewrites.push(pp_mitm::RewriteRule { pattern, kind });
}

/// Surge / Loon `[Header Rewrite]` line parsing.
///
/// 可带 `http-request` / `http-response` 阶段前缀（Surge 4.x 语法），缺省按 Request 阶段
/// （兼容旧解析行为）。`header-replace` / `header-add` → `HeaderRewrite`（set），
/// `header-del` → `HeaderRewrite`（删除），`header-replace-regex Name <regex> <repl>` →
/// `HeaderValueRewrite`。
pub(super) fn parse_surge_header_rewrite(cfg: &mut super::ImportedConfig, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 3 {
        cfg.warn("header rewrite", line, "unrecognized line");
        return;
    }
    let (phase, tokens) = match tokens[0] {
        "http-request" => (Phase::Request, &tokens[1..]),
        "http-response" => (Phase::Response, &tokens[1..]),
        _ => (Phase::Request, &tokens[..]),
    };
    if tokens.len() < 3 {
        cfg.warn("header rewrite", line, "unrecognized line");
        return;
    }
    let Some(pattern) = compile_pattern(tokens[0], cfg, "header rewrite", line) else {
        return;
    };
    match tokens[1] {
        "header-replace" | "header-add" => {
            let name = tokens[2].to_string();
            let value = Some(tokens[3..].join(" "));
            cfg.rewrites.push(pp_mitm::RewriteRule {
                pattern,
                kind: RewriteKind::HeaderRewrite { phase, name, value },
            });
        }
        "header-del" => {
            let name = tokens[2].to_string();
            cfg.rewrites.push(pp_mitm::RewriteRule {
                pattern,
                kind: RewriteKind::HeaderRewrite {
                    phase,
                    name,
                    value: None,
                },
            });
        }
        "header-replace-regex" => {
            if tokens.len() < 5 {
                cfg.warn(
                    "header rewrite",
                    line,
                    "header-replace-regex missing name/regex/replacement",
                );
                return;
            }
            let Some(value_pattern) = compile_pattern(tokens[3], cfg, "header rewrite", line)
            else {
                return;
            };
            cfg.rewrites.push(pp_mitm::RewriteRule {
                pattern,
                kind: RewriteKind::HeaderValueRewrite {
                    phase,
                    name: tokens[2].to_string(),
                    value_pattern,
                    replacement: tokens[4..].join(" "),
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

/// Surge `[Body Rewrite]` section line parsing:
/// `http-request|http-response pattern <regex> <replacement>` → `BodyRewrite{body_pattern}`。
///
/// `jq` 形态（`http-response pattern jq <expr>`）无引擎可表达，记 warning 跳过。
pub(super) fn parse_surge_body_rewrite(cfg: &mut super::ImportedConfig, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 4 {
        cfg.warn("body rewrite", line, "unrecognized line");
        return;
    }
    let phase = match tokens[0] {
        "http-request" => Phase::Request,
        "http-response" => Phase::Response,
        other => {
            cfg.warn(
                "body rewrite",
                line,
                &format!("unrecognized phase '{other}'"),
            );
            return;
        }
    };
    let Some(pattern) = compile_pattern(tokens[1], cfg, "body rewrite", line) else {
        return;
    };
    if tokens[2] == "jq" {
        cfg.warn(
            "body rewrite",
            line,
            "jq body rewrite not supported, skipped",
        );
        return;
    }
    let Some(body_pattern) = compile_pattern(tokens[2], cfg, "body rewrite", line) else {
        return;
    };
    cfg.rewrites.push(pp_mitm::RewriteRule {
        pattern,
        kind: RewriteKind::BodyRewrite {
            phase,
            body_pattern: Some(body_pattern),
            replacement: tokens[3..].join(" "),
        },
    });
}

/// Surge / Loon `[Map Local]` line parsing:
/// `pattern data="..." data-type=text status-code=200 header="Name:value"` → `Mock`.
///
/// `data-type` / `mime-type` mapped to Content-Type response header (`text` → `text/plain`,
/// `json` → `application/json`, `html` → `text/html`, `css` → `text/css`,
/// `js`/`javascript` → `application/javascript`, `xml` → `application/xml`;
/// when the value itself contains `/`, treated as full Content-Type preserved as-is).
/// `header="Name:value"` can repeat, split by first `:` into response header.
/// Only when `header=` does not explicitly specify Content-Type is the mapped Content-Type from
/// `data-type` appended (explicit takes priority).
pub(super) fn parse_surge_map_local(cfg: &mut super::ImportedConfig, line: &str) {
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
    let mut headers: Vec<(String, String)> = Vec::new();
    let mut data_type: Option<String> = None;
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
            "data-type" | "mime-type" => data_type = Some(strip_quotes(value).to_string()),
            "header" => {
                let header = strip_quotes(value);
                match header.split_once(':') {
                    Some((name, value)) => {
                        let (name, value) = (name.trim(), value.trim());
                        if name.is_empty() {
                            cfg.warn("map local", line, &format!("invalid header '{header}'"));
                        } else {
                            headers.push((name.to_string(), value.to_string()));
                        }
                    }
                    None => cfg.warn("map local", line, &format!("invalid header '{header}'")),
                }
            }
            other => cfg.warn(
                "map local",
                line,
                &format!("unrecognized parameter '{other}'"),
            ),
        }
    }
    // `data-type` / `mime-type` → Content-Type; only appended when `header=` does not explicitly specify.
    if let Some(dt) = data_type {
        let has_explicit_content_type = headers
            .iter()
            .any(|(n, _)| n.eq_ignore_ascii_case("content-type"));
        if !has_explicit_content_type {
            match content_type_for_data_type(&dt) {
                Some(ct) => headers.push(("Content-Type".to_string(), ct)),
                None => cfg.warn("map local", line, &format!("unknown data-type '{dt}'")),
            }
        }
    }
    let Some(body) = data else {
        cfg.warn("map local", line, "missing 'data' parameter");
        return;
    };
    cfg.rewrites.push(pp_mitm::RewriteRule {
        pattern,
        kind: RewriteKind::Mock {
            status,
            body: body.into_bytes(),
            headers,
        },
    });
}

/// Map Surge/Loon `data-type` / `mime-type` value to HTTP Content-Type.
///
/// Known aliases return standard MIME; when the value itself contains `/`, treated as full
/// Content-Type returned as-is; others return `None` (caller records warning).
fn content_type_for_data_type(data_type: &str) -> Option<String> {
    match data_type.trim().to_ascii_lowercase().as_str() {
        "text" => Some("text/plain".to_string()),
        "json" => Some("application/json".to_string()),
        "html" => Some("text/html".to_string()),
        "css" => Some("text/css".to_string()),
        "js" | "javascript" => Some("application/javascript".to_string()),
        "xml" => Some("application/xml".to_string()),
        other if other.contains('/') => Some(other.to_string()),
        _ => None,
    }
}
