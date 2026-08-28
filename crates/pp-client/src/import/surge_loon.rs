//! Surge / Loon dialect parsing (`[Script]`, `[URL Rewrite]`, `[Header Rewrite]`,
//! `[Map Local]`, `[Argument]`).

use pp_mitm::{Phase, RewriteKind};
use pp_script::{ScriptDialect, ScriptKind};

use super::utils::*;
use super::{ArgKind, ArgSpec, ConfigMeta};

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

/// Loon `[Script]` line parsing: `http-request|http-response ^pattern param=value,...`.
///
/// Loon vs Surge syntax differences: type is the first token, no `name =` prefix,
/// script name taken from `tag=`, `argument=` parameter preserved as-is (can contain
/// `[{key},...]` template), `engine` / `binary-body-mode` and other parameters ignored.
pub(super) fn parse_loon_script(cfg: &mut super::ImportedConfig, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        cfg.warn(
            "script",
            line,
            "unrecognized line (expected '<type> ^pattern params...')",
        );
        return;
    }
    let kind = match tokens[0] {
        "http-request" => ScriptKind::HttpRequest,
        "http-response" => ScriptKind::HttpResponse,
        other => {
            cfg.warn(
                "script",
                line,
                &format!("unrecognized script type '{other}'"),
            );
            return;
        }
    };
    let pattern_src = tokens[1];
    let params = parse_kv_params(&tokens[2..].join(" "));
    let Some(pattern) = compile_pattern(pattern_src, cfg, "script", line) else {
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
    let name = params
        .get("tag")
        .map(|t| strip_quotes(t).to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| derive_name_from_url(script_path));
    let requires_body = parse_bool(
        params
            .get("requires-body")
            .or_else(|| params.get("require-body")),
    );
    // `argument=` parameter preserved as-is (template placeholder replaced at runtime);
    // default max_size matches Surge.
    let argument = params.get("argument").cloned();
    cfg.script_urls.push((name.clone(), script_path.clone()));
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

/// Loon `[Argument]` section line parsing: `Key = input/select,"default","opt2",...,tag=...,desc=...`.
///
/// `input` followed by a single quoted default value; `select` followed by first quoted value as
/// default, rest as options; `tag=` as separate field, `desc=` goes to description. Results merged
/// into [`ConfigMeta::arguments`] by key (when `#!arguments=` already declared,补齐 fields).
pub(super) fn parse_loon_argument(cfg: &mut super::ImportedConfig, line: &str) {
    let Some(eq) = line.find('=') else {
        cfg.warn(
            "argument",
            line,
            "unrecognized line (expected 'Key = kind,...')",
        );
        return;
    };
    let key = line[..eq].trim();
    if key.is_empty() {
        cfg.warn("argument", line, "empty argument key");
        return;
    }
    let segments = split_kv_segments(line[eq + 1..].trim());
    let Some(kind_src) = segments.first() else {
        cfg.warn("argument", line, "missing argument kind");
        return;
    };
    let kind = match kind_src.trim().to_ascii_lowercase().as_str() {
        "input" => ArgKind::Input,
        "select" => ArgKind::Select,
        other => {
            cfg.warn(
                "argument",
                line,
                &format!("unrecognized argument kind '{other}'"),
            );
            return;
        }
    };
    // Remaining segments: quoted values (default / options) and tag= / desc= parameters.
    let mut values: Vec<String> = Vec::new();
    let mut tag: Option<String> = None;
    let mut desc: Option<String> = None;
    for seg in &segments[1..] {
        let seg = seg.trim();
        if seg.is_empty() {
            continue;
        }
        // Quoted values checked before key=value, to avoid `=` inside values (e.g., `"a=b"`) being misjudged.
        if seg.starts_with('"') {
            values.push(strip_quotes(seg).to_string());
            continue;
        }
        if let Some((k, v)) = seg.split_once('=') {
            match k.trim().to_ascii_lowercase().as_str() {
                "tag" => tag = Some(strip_quotes(v.trim()).to_string()),
                "desc" => desc = Some(strip_quotes(v.trim()).to_string()),
                _ => {} // other key=value (e.g., enable) ignored
            }
        }
    }
    let (default_value, options) = match kind {
        ArgKind::Select => {
            let mut it = values.into_iter();
            (it.next().unwrap_or_default(), it.collect())
        }
        ArgKind::Input => (values.into_iter().next().unwrap_or_default(), Vec::new()),
    };
    merge_argument_spec(
        &mut cfg.meta,
        ArgSpec {
            key: key.to_string(),
            default_value,
            description: desc,
            kind,
            options,
            tag,
        },
    );
}

/// Merge Loon `[Argument]` section parsed parameter declaration into [`ConfigMeta`] by key.
///
/// When `#!arguments=` (or `#!arguments-desc=`) already declared the same key,补齐 new fields;
/// otherwise append the whole spec.
pub(super) fn merge_argument_spec(meta: &mut ConfigMeta, spec: ArgSpec) {
    if let Some(existing) = meta.arguments.iter_mut().find(|a| a.key == spec.key) {
        existing.kind = spec.kind;
        if !spec.default_value.is_empty() {
            existing.default_value = spec.default_value;
        }
        if spec.description.is_some() {
            existing.description = spec.description;
        }
        if !spec.options.is_empty() {
            existing.options = spec.options;
        }
        if spec.tag.is_some() {
            existing.tag = spec.tag;
        }
    } else {
        meta.arguments.push(spec);
    }
}

/// Surge / Loon `[URL Rewrite]` line parsing: `pattern target [header-arg]` → `UrlRewrite`.
///
/// Third segment request-header parameter cannot be expressed, deviation recorded.
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
    if tokens.len() > 2 {
        cfg.warn(
            "url rewrite",
            line,
            "request-header argument cannot be expressed, only URL rewritten",
        );
    }
    cfg.rewrites.push(pp_mitm::RewriteRule {
        pattern,
        kind: RewriteKind::UrlRewrite {
            target: tokens[1].to_string(),
        },
    });
}

/// Surge / Loon `[Header Rewrite]` line parsing:
/// `pattern header-replace Name Value` / `pattern header-del Name` → `HeaderRewrite{Request}`.
pub(super) fn parse_surge_header_rewrite(cfg: &mut super::ImportedConfig, line: &str) {
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
            cfg.rewrites.push(pp_mitm::RewriteRule {
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
            cfg.rewrites.push(pp_mitm::RewriteRule {
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
            body,
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
