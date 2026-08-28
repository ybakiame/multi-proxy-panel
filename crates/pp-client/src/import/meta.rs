//! Config header metadata parsing (`#!key=value` lines).
//!
//! Common in Surge `.sgmodule`, QX `.conf`, and Loon config headers.

use serde::{Deserialize, Serialize};

/// File header `#!key=value` metadata.
///
/// All fields are `Option`: missing keys remain `None`; camelCase keys like `openUrl`
/// are normalized to snake_case fields. `#[serde(default)]` ensures missing keys round-trip
/// as `None` during serialization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigMeta {
    /// `#!name`: config name.
    pub name: Option<String>,
    /// `#!desc` (alias `#!description`): config description; when both exist `#!desc` takes priority.
    pub desc: Option<String>,
    /// `#!author`: author.
    pub author: Option<String>,
    /// `#!icon`: icon URL.
    pub icon: Option<String>,
    /// `#!date`: release date.
    pub date: Option<String>,
    /// `#!category`: category tag.
    pub category: Option<String>,
    /// `#!openUrl`: associated link (e.g., App Store).
    pub open_url: Option<String>,
    /// `#!arguments=` / `#!arguments-desc=` declared module parameters (key / default / description).
    pub arguments: Vec<ArgSpec>,
}

/// Loon `[Argument]` section declared parameter control type (`#!arguments=` defaults to input).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArgKind {
    /// Free text input (`input`).
    #[default]
    Input,
    /// Dropdown select (`select`).
    Select,
}

/// Surge/Loon module `#!arguments=` declared single parameter.
///
/// `argument="{key}"` template placeholders are replaced at runtime by
/// "user value → default value → keep as-is". Loon `[Argument]` section
/// (`Key = input/select,"default","opt2",...,tag=...,desc=...`)
/// additionally provides [`ArgSpec::kind`] (control type) and [`ArgSpec::options`] (select options).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArgSpec {
    /// Parameter key (used by template placeholder `{key}`, `#!arguments= key:default`).
    pub key: String,
    /// Default value (replaced into argument template when user has not configured this key; can be empty string).
    pub default_value: String,
    /// Parameter description (`#!arguments-desc=` / Loon `desc=`; optional).
    pub description: Option<String>,
    /// Control type (Loon `[Argument]` section provides; `#!arguments=` defaults to `Input`).
    #[serde(default)]
    pub kind: ArgKind,
    /// `select` control options (first quoted value is [`ArgSpec::default_value`], rest are options).
    #[serde(default)]
    pub options: Vec<String>,
    /// Parameter group tag (Loon `[Argument]` section `tag=`; optional).
    #[serde(default)]
    pub tag: Option<String>,
}

/// Parse consecutive file header `#!key=value` metadata lines; stops at the first non-`#!` line
/// (including blank lines).
///
/// Unknown keys and malformed lines are silently ignored (metadata does not affect rule parsing).
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
                // `#!description=` is an alias for `#!desc=`; `#!desc=` already filled (or appeared first)
                // so keep desc priority.
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
            _ => {} // unknown key ignored
        }
    }
    meta
}

/// Parse `#!arguments= key:default, key2:default2` declaration: comma-separated, values may contain
/// spaces (trimmed).
///
/// Comma splitting is quote-aware (`Types:"Translate,External"` commas inside quotes are not split);
/// values may be quoted (`key:"default"`, quotes stripped) or bare (`key:default`). Segments without
/// `:` (e.g., `#!arguments= foo`) are silently skipped (no default value means no declaration).
pub(super) fn parse_arguments_decl(value: &str) -> Vec<ArgSpec> {
    let mut args = Vec::new();
    for pair in super::utils::split_kv_segments(value) {
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
            default_value: super::utils::strip_quotes(default_value.trim()).to_string(),
            description: None,
            ..ArgSpec::default()
        });
    }
    args
}

/// Parse `#!arguments-desc= {key:"description", ...}` and merge into the argument declaration list
/// by key.
///
/// First tries strict JSON object; falls back to `key:"value"` / `key:'value'` loose extraction.
/// Keys appearing only in description (not in declarations) are padded with empty default values.
pub(super) fn merge_argument_descriptions(args: &mut Vec<ArgSpec>, value: &str) {
    let pairs = parse_desc_pairs(value);
    for (key, desc) in pairs {
        if let Some(spec) = args.iter_mut().find(|a| a.key == key) {
            spec.description = Some(desc);
        } else {
            args.push(ArgSpec {
                key,
                default_value: String::new(),
                description: Some(desc),
                ..ArgSpec::default()
            });
        }
    }
}

/// Extract `(key, desc)` pairs from `#!arguments-desc=` (strict JSON → loose syntax → naive fallback).
pub(super) fn parse_desc_pairs(value: &str) -> Vec<(String, String)> {
    // 1. Strict JSON object: `{"key": "desc", ...}`.
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(value)
        && let Some(obj) = json.as_object()
    {
        return obj
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect();
    }
    // 2. Loose syntax: `key:"description"` / `key:'description'` (unquoted keys, values with spaces/Chinese ok).
    let mut out = Vec::new();
    let Ok(re) = regex::Regex::new(r#"([A-Za-z0-9_.\-]+)\s*:\s*("([^"]*)"|'([^']*)')"#) else {
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
    // 3. Naive syntax: `key:description` (no `{}`, no quotes, description can contain spaces/Chinese,
    //    commas separate multiple). `regex` crate does not support look-around, so manual scan:
    //    only split at commas followed immediately by `key:`.
    parse_naive_desc_pairs(value)
}

/// Naive `#!arguments-desc=` syntax `key:description[, key2:description2, ...]` pair extraction.
///
/// Description has no quotes, can contain Chinese/spaces/colons/commas; only splits at commas
/// followed by a valid key + colon.
pub(super) fn parse_naive_desc_pairs(value: &str) -> Vec<(String, String)> {
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
        // Locate description end: next comma followed immediately by `key:`; otherwise extend to end.
        let mut seg_end = desc_src.len();
        let mut next_start: Option<usize> = None;
        for (offset, c) in desc_src.char_indices() {
            if c == ',' {
                let after = desc_src[offset + 1..].trim_start();
                if let Some(c2) = after.find(':')
                    && is_valid_key(&after[..c2])
                {
                    seg_end = offset;
                    next_start = Some(offset + 1);
                    break;
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
