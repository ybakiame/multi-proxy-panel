//! Loon dialect parsing (`[Script]`, `[Rewrite]`, `[Argument]`).

use pp_mitm::{Phase, RewriteKind};
use pp_script::ScriptKind;

use super::utils::*;
use super::{ArgKind, ArgSpec, ConfigMeta};

/// Loon `[Script]` line parsing: `http-request|http-response ^pattern param=value,...`.
///
/// Loon vs Surge syntax differences: type is the first token, no `name =` prefix,
/// script name taken from `tag=`, `argument=` parameter percent-decoded (常为 URL 编码
/// JSON), `engine` / `binary-body-mode` and other parameters ignored.
/// 兼容参数与 pattern 逗号粘连的生态写法（`^pat,requires-body=0 script-path=...`）。
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
    // 生态中存在参数与 pattern 逗号粘连的写法（`^pat,requires-body=0 script-path=...`），
    // 拆出 pattern 后把粘连参数并入 kv 解析。
    let (pattern_src, inline_params) = match tokens[1].split_once(',') {
        Some((pattern, rest)) => (pattern, rest.to_string()),
        None => (tokens[1], String::new()),
    };
    let param_src = if inline_params.is_empty() {
        tokens[2..].join(" ")
    } else {
        // 粘连参数与后续 token 之间是空格分隔；拼入 kv 流时用逗号衔接，避免
        // `requires-body=0 script-path=...` 被读成 requires-body 的值。
        format!("{},{}", inline_params, tokens[2..].join(" "))
    };
    let params = parse_kv_params(&param_src);
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
    // `argument=` 常为 URL 编码的 JSON，解码后透传；无 `%` 时原样返回。
    // `max-size=-1`（Loon 不限）映射到 10MB 上限，语义同 Surge。
    let argument = params.get("argument").map(|a| percent_decode(a));
    let max_size = parse_max_size(params.get("max-size")).unwrap_or(131072);
    cfg.script_urls.push((name.clone(), script_path.clone()));
    cfg.scripts.push(pp_mitm::ScriptRule {
        name,
        kind,
        pattern,
        requires_body,
        max_size,
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

/// Loon `[Rewrite]` section line parsing:
/// `pattern reject[-200|-img|-dict|-array]` / `pattern 302|307 target` /
/// `pattern header-del|header-replace|header-add Name [value]` /
/// `pattern header-replace-regex Name <regex> <repl>`。
///
/// Loon `[Rewrite]` 的 header 操作作用于请求阶段。
pub(super) fn parse_loon_rewrite(cfg: &mut super::ImportedConfig, line: &str) {
    let tokens: Vec<&str> = line.split_whitespace().collect();
    if tokens.len() < 2 {
        cfg.warn("rewrite", line, "unrecognized line");
        return;
    }
    let Some(pattern) = compile_pattern(tokens[0], cfg, "rewrite", line) else {
        return;
    };
    let kind = match tokens[1] {
        "reject" => RewriteKind::reject(),
        "reject-200" => RewriteKind::reject_200(),
        "reject-img" => RewriteKind::reject_img(),
        "reject-dict" | "reject-json" => RewriteKind::reject_json("{}"),
        "reject-array" => RewriteKind::reject_json("[]"),
        mode @ ("302" | "307") => {
            let Some(target) = tokens.get(2) else {
                cfg.warn("rewrite", line, "missing redirect target");
                return;
            };
            RewriteKind::Redirect {
                status: if mode == "307" { 307 } else { 302 },
                target: (*target).to_string(),
            }
        }
        "header-del" => {
            let Some(name) = tokens.get(2) else {
                cfg.warn("rewrite", line, "header-del missing header name");
                return;
            };
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name: (*name).to_string(),
                value: None,
            }
        }
        "header-replace" | "header-add" => {
            let Some(name) = tokens.get(2) else {
                cfg.warn("rewrite", line, "missing header name");
                return;
            };
            RewriteKind::HeaderRewrite {
                phase: Phase::Request,
                name: (*name).to_string(),
                value: Some(tokens[3..].join(" ")),
            }
        }
        "header-replace-regex" => {
            if tokens.len() < 5 {
                cfg.warn(
                    "rewrite",
                    line,
                    "header-replace-regex missing name/regex/replacement",
                );
                return;
            }
            let Some(value_pattern) = compile_pattern(tokens[3], cfg, "rewrite", line) else {
                return;
            };
            RewriteKind::HeaderValueRewrite {
                phase: Phase::Request,
                name: tokens[2].to_string(),
                value_pattern,
                replacement: tokens[4..].join(" "),
            }
        }
        other => {
            cfg.warn(
                "rewrite",
                line,
                &format!("unrecognized rewrite action '{other}'"),
            );
            return;
        }
    };
    cfg.rewrites.push(pp_mitm::RewriteRule { pattern, kind });
}
