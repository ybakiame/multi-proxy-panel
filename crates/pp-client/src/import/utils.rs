//! Parsing utilities shared across import dialects.

use regex::Regex;
use std::collections::HashMap;

/// Compile regex; on failure record warning and return `None` (no panic).
pub(super) fn compile_pattern(
    src: &str,
    cfg: &mut super::ImportedConfig,
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

/// Whether the string is a remote http(s) script URL; others (local paths, etc.) are not supported.
pub(super) fn is_remote_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Derive task name from script URL: take the last path segment filename (strip `.js` suffix),
/// fallback to the whole URL on failure.
pub(super) fn derive_name_from_url(url: &str) -> String {
    if let Ok(parsed) = reqwest::Url::parse(url)
        && let Some(seg) = parsed.path_segments().and_then(|mut it| it.next_back())
        && !seg.is_empty()
    {
        let stem = seg.strip_suffix(".js").unwrap_or(seg);
        return stem.to_string();
    }
    url.to_string()
}

/// Strip leading/trailing paired double quotes (`"* * * * *"` → `* * * * *`);
/// only strips when both ends are quotes (`Types="a"&Vendor="b"` where the whole is not quoted
/// is kept as-is).
pub(super) fn strip_quotes(s: &str) -> &str {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

/// Parse `key=value` value (`true` / `1` / `yes` treated as true, default false).
///
/// Surge's `requires-body` accepts both boolean and numeric forms: `true/false` and `1/0`.
pub(super) fn parse_bool(v: Option<&String>) -> bool {
    matches!(
        v.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("true") | Some("1") | Some("yes")
    )
}

/// Surge `max-size` unlimited value mapped approximate upper bound (10MB).
///
/// In Surge semantics `max-size=-1` / `max-size=0` means "unlimited body size";
/// pp-mitm's `max_size` is `usize`, mapping `usize::MAX` directly would break internal buffering
/// semantics, so mapped to 10MB upper bound (sufficient for绝大多数 real response bodies).
const MAX_SIZE_UNLIMITED: usize = 10 * 1024 * 1024;

/// Parse Surge `max-size`: `-1` / `0` (unlimited) → [`MAX_SIZE_UNLIMITED`], normal numbers parsed as-is.
pub(super) fn parse_max_size(v: Option<&String>) -> Option<usize> {
    let s = v?.trim();
    match s {
        "-1" | "0" => Some(MAX_SIZE_UNLIMITED),
        s => s.parse::<usize>().ok(),
    }
}

/// Split by commas in `key=value` / `key:value` segments, but skip commas inside quotes and
/// `[` `]` / `{` `}` (`Types:"Translate,External"`, `argument=[{a},{b}]`, regex quantifiers
/// `\d{1,2}` are not mistakenly split).
pub(super) fn split_kv_segments(input: &str) -> Vec<String> {
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

/// Parse comma-separated `key=value` parameter list; keys normalized to lowercase, values unquoted.
pub(super) fn parse_kv_params(input: &str) -> HashMap<String, String> {
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

/// Split by whitespace but preserve whitespace inside double quotes (e.g., `data="hello world"`).
pub(super) fn split_tokens_keep_quoted(line: &str) -> Vec<String> {
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

/// Comment (`#` / `;` prefix) or blank line.
pub(super) fn is_comment_or_blank(line: &str) -> bool {
    line.is_empty() || line.starts_with('#') || line.starts_with(';')
}

/// Parse `[section]` header, normalized to lowercase + single space between words
/// (`[URL Rewrite]` → `"url rewrite"`).
pub(super) fn section_name(line: &str) -> Option<String> {
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
