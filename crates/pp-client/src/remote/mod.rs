//! Remote resource subscription (script / rewrite rule URL periodic fetching).
//!
//! Manages user-configured remote subscriptions in the client: pure JS scripts
//! ([`RemoteKind::Script`]) and QX / Surge / Loon config snippets ([`RemoteKind::Snippet`],
//! parsed via [`parse_import`]).
//!
//! Fetch results are cached to disk:
//! - Script → `data_dir/scripts/<name>.js`
//! - Snippet → parsed + backfilled script URL sources aggregated into
//!   `data_dir/remote_cache/<name>.json` (rewrite rule `Regex` persisted as pattern string,
//!   recompiled on readback)
//!
//! At runtime, [`RemoteManager::load_cached`] reads all caches and merges them into
//! [`MergedRemoteConfig`] for MITM (rewrite / script hooks / hostnames) and the task scheduler
//! (task scripts).

use std::collections::HashMap;

use pp_mitm::{RewriteRule, ScriptRule};
use pp_script::{ScriptDialect, TaskScript};
use serde::{Deserialize, Serialize};

use crate::import::{ArgSpec, ConfigMeta};

mod cache;
mod manager;
mod notify;

#[cfg(test)]
mod tests;

pub use cache::*;
pub use manager::*;
pub use notify::*;

/// A single remote subscription resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteResource {
    /// Resource name (script filename / cache filename).
    pub name: String,
    /// Remote URL (http/https).
    pub url: String,
    /// Resource type.
    pub kind: RemoteKind,
    /// Snippet dialect.
    pub dialect: ScriptDialect,
    /// Update interval (seconds), default 86400 (daily).
    pub update_interval_secs: u64,
    /// Whether enabled, default enabled.
    pub enabled: bool,
    /// Resource description (optional; new field, missing in old manifests defaults to `None`).
    pub description: Option<String>,
    /// User-configured parameter values `(key, value)` (keys correspond to `#!arguments=` declarations;
    /// missing in old manifests defaults to empty).
    #[serde(default)]
    pub argument_values: Vec<(String, String)>,
    /// Resource icon URL (optional; pre-filled from sniff result when creating new resource;
    /// missing in old manifests defaults to `None`).
    #[serde(default)]
    pub icon: Option<String>,
    /// Module parameter declarations (`#!arguments=` / Loon `[Argument]` section; populated from
    /// frontend detect meta when adding, backfilled from fetched meta when manifest has no declaration;
    /// missing in old manifests defaults to empty).
    #[serde(default)]
    pub arguments: Vec<ArgSpec>,
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
            arguments: Vec::new(),
        }
    }
}

/// Pre-fill `argument_values` from [`RemoteResource::arguments`] defaults (called when adding/updating
/// a resource); user-configured keys (already in `argument_values`) are not overwritten.
pub fn prefill_argument_values(remote: &mut RemoteResource) {
    for spec in &remote.arguments {
        if !remote.argument_values.iter().any(|(k, _)| k == &spec.key) {
            remote
                .argument_values
                .push((spec.key.clone(), spec.default_value.clone()));
        }
    }
}

/// Remote resource type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteKind {
    /// Pure JS script file, written to `scripts/<name>.js`.
    Script,
    /// QX / Surge / Loon config snippet, parsed and aggregated after caching.
    Snippet,
}

/// Sniff remote resource type and script dialect from URL suffix.
///
/// - `.sgmodule` → `(Snippet, Surge)`
/// - `.plugin` / `.loon` → `(Snippet, Loon)`
/// - `.conf` → `(Snippet, QuantumultX)`
/// - `.js` → `(Script, QuantumultX)` (QX-style scripts most common, default QX)
/// - Other / no suffix → `None`
///
/// Suffix determination ignores query / fragment (`?token=...` etc. does not affect suffix).
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

/// A single `fetch_all` fetch report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FetchReport {
    /// Successfully fetched and cached remote resources (each enabled remote counts once).
    pub fetched: usize,
    /// Written pure JS scripts (Script) or backfilled script hooks (Snippet).
    pub scripts: usize,
    /// Rewrite rules parsed from Snippet.
    pub rewrites: usize,
    /// Task scripts parsed from Snippet.
    pub tasks: usize,
    /// Fetch/parse/backfill failure and deviation warnings.
    pub warnings: Vec<String>,
}

/// Summary of a single local import merged into cache.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Rewrite rules merged into cache.
    pub rewrites: usize,
    /// Script hooks merged into cache (only when `source` is non-empty).
    pub scripts: usize,
    /// Task scripts merged into cache (only when `source` is non-empty).
    pub tasks: usize,
    /// Hostnames contributed by this import.
    pub hostnames: usize,
    /// Skipped (source empty not fetched) and parse deviation warnings.
    pub warnings: Vec<String>,
    /// Config header metadata (`#!key=value`) parsed from this import.
    pub meta: ConfigMeta,
}

/// Runtime config after merging all remote subscription caches.
#[derive(Default)]
pub struct MergedRemoteConfig {
    /// Rewrite rules (patterns recompiled).
    pub rewrites: Vec<RewriteRule>,
    /// Script hook rules (`source` backfilled).
    pub scripts: Vec<ScriptRule>,
    /// Task scripts (`source` backfilled).
    pub task_scripts: Vec<TaskScript>,
    /// MITM hostname whitelist (deduplicated).
    pub hostnames: Vec<String>,
    /// Config header metadata from each remote resource (including `#!arguments` declarations and defaults).
    pub metas: Vec<ConfigMeta>,
}

/// Replace Surge/Loon module `argument=` template placeholders `{key}` / `{{{key}}}` with concrete values.
///
/// Priority: user-configured values (`user_values`, key→value) → parameter declaration defaults
/// (`defaults`, key→default) → keep original placeholder if none. Both `{{{key}}}` (Surge standard
/// triple-brace placeholder) and `{key}` are supported, with long form matched first to avoid
/// short-form replacement polluting triple-brace placeholders. Return value is the string injected
/// into JS `$argument`.
pub fn resolve_argument_template(
    template: &str,
    user_values: &HashMap<String, String>,
    defaults: &HashMap<String, String>,
) -> String {
    // User values override defaults (duplicate keys use user config).
    let mut values: HashMap<String, String> = HashMap::new();
    values.extend(defaults.iter().map(|(k, v)| (k.clone(), v.clone())));
    values.extend(user_values.iter().map(|(k, v)| (k.clone(), v.clone())));

    let mut out = template.to_string();
    // Replace long form `{{{key}}}` first, then short form `{key}`;
    // undeclared placeholders (no value) keep original.
    for (key, value) in &values {
        out = out.replace(&placeholder(key, true), value);
    }
    for (key, value) in &values {
        out = out.replace(&placeholder(key, false), value);
    }
    out
}

/// Construct placeholder literal: `triple=true` generates `{{{key}}}` (Surge triple-brace standard),
/// otherwise generates `{key}` (short form).
fn placeholder(key: &str, triple: bool) -> String {
    let (open, close) = if triple { ("{{{", "}}}") } else { ("{", "}") };
    format!("{open}{key}{close}")
}

/// Apply argument template replacement to cached merged script hook rules: `{key}` / `{{{key}}}` →
/// user values → parameter declaration defaults → keep original (see [`resolve_argument_template`]).
///
/// `remotes` provides user-configured parameter values ([`RemoteResource::argument_values`]),
/// `metas` provides each resource's `#!arguments=` declared keys and defaults ([`ConfigMeta::arguments`]).
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
                rule.argument = Some(resolve_argument_template(&template, &user_values, &defaults));
            }
            rule
        })
        .collect()
}

/// Filename safety: replace path separators with `_` to prevent directory traversal.
pub(crate) fn safe_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect()
}

/// Allowed icon cache extension whitelist.
pub(crate) const ICON_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "webp", "gif", "svg", "ico"];

/// Get icon extension from URL path suffix (ignoring query / fragment, case-insensitive);
/// returns `None` when suffix is not in [`ICON_EXTENSIONS`].
pub(crate) fn icon_ext_from_url(url: &str) -> Option<&'static str> {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)?;
    match ext.as_str() {
        "png" => Some("png"),
        "jpg" => Some("jpg"),
        "jpeg" => Some("jpeg"),
        "webp" => Some("webp"),
        "gif" => Some("gif"),
        "svg" => Some("svg"),
        "ico" => Some("ico"),
        _ => None,
    }
}

/// Infer icon format from response bytes (equivalent to inferring from Content-Type: `fetch_resource_bytes`
/// only returns bytes without response headers, so identify common image formats by file signature).
/// Returns `None` when unrecognized.
pub(crate) fn icon_ext_from_bytes(bytes: &[u8]) -> Option<&'static str> {
    // PNG: 89 50 4E 47
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        return Some("png");
    }
    // JPEG: FF D8 FF
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpg");
    }
    // GIF: GIF8
    if bytes.starts_with(b"GIF8") {
        return Some("gif");
    }
    // WebP: RIFF....WEBP
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("webp");
    }
    // ICO: 00 00 01 00
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return Some("ico");
    }
    // SVG is text: skip leading whitespace, then check for `<?xml` / `<svg` prefix.
    let head = bytes.iter().take_while(|b| b.is_ascii_whitespace()).count();
    let head_bytes = &bytes[head.min(bytes.len())..bytes.len().min(head + 256)];
    if head_bytes.starts_with(b"<?xml") || head_bytes.starts_with(b"<svg") {
        return Some("svg");
    }
    None
}
