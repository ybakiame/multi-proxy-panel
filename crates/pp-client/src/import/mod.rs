//! Third-party config snippet import (QX / Surge / Loon → pp-mitm / pp-script rules).
//!
//! This is the common foundation for "remote subscription (2.3b)" and "config import (2.4)":
//! parses Quantumult X / Surge / Loon rewrite / script / task / mitm config snippets into
//! pp-mitm [`RewriteRule`] / script hook rules and pp-script [`TaskScript`].
//!
//! Design trade-offs:
//! - Only covers commonly used subsets; unknown lines are skipped and recorded in
//!   [`ImportedConfig::warnings`] (comments / blank lines are silently skipped);
//! - Scripts are always treated as remote http(s) URLs: `source` is left empty, and the URL
//!   is recorded in [`ImportedConfig::script_urls`] / [`ImportedConfig::task_scripts`],
//!   to be fetched and backfilled by the caller;
//! - When pp-mitm fields are structurally incompatible with ecosystem syntax, existing fields
//!   are approximated and a warning is recorded (pp-mitm itself is not modified).

#[cfg(feature = "mitm")]
use pp_common::PanelResult;
#[cfg(feature = "mitm")]
use pp_mitm::{RewriteRule, ScriptRule as HookScriptRule};
#[cfg(feature = "mitm")]
use pp_script::ScriptDialect;
#[cfg(feature = "mitm")]
use pp_script::TaskScript;

#[cfg(all(test, feature = "mitm"))]
use pp_mitm::Phase;
#[cfg(all(test, feature = "mitm"))]
use pp_script::ScriptKind;

mod meta;
#[cfg(feature = "mitm")]
mod qx;
#[cfg(feature = "mitm")]
mod surge_loon;
mod utils;

pub use meta::*;
#[cfg(feature = "mitm")]
use qx::*;
#[cfg(feature = "mitm")]
use surge_loon::*;
#[cfg(feature = "mitm")]
use utils::*;

/// Result of a single import parse.
#[cfg(feature = "mitm")]
#[derive(Default)]
pub struct ImportedConfig {
    /// URL / Header / Body rewrite and Reject / Mock rules.
    pub rewrites: Vec<RewriteRule>,
    /// Script hook rules; `source` is empty, corresponding script URLs are in
    /// [`ImportedConfig::script_urls`].
    pub scripts: Vec<HookScriptRule>,
    /// Script hook remote addresses `(script_name, URL)`, corresponding to `scripts` in order.
    pub script_urls: Vec<(String, String)>,
    /// Scheduled tasks (`TaskScript.source` is empty) and their script URLs.
    pub task_scripts: Vec<(TaskScript, String)>,
    /// MITM hostname whitelist.
    pub hostnames: Vec<String>,
    /// Unrecognized lines / mapping deviations that cannot be expressed.
    pub warnings: Vec<String>,
    /// File header `#!key=value` metadata (see [`ConfigMeta`]).
    pub meta: ConfigMeta,
}

#[cfg(feature = "mitm")]
impl ImportedConfig {
    /// Record a warning: `tracing::warn` while also writing to the `warnings` list.
    fn warn(&mut self, section: &str, line: &str, msg: &str) {
        let text = format!("[{section}] {msg}: {line}");
        tracing::warn!(section, "{text}");
        self.warnings.push(text);
    }
}

/// Parse QX / Surge / Loon config snippets.
///
/// `dialect` is specified by the caller to indicate the source software, which determines
/// [`TaskScript::dialect`] and other dialect markers; syntactically isomorphic grammars
/// (e.g., Surge / Loon `[Script]`) share the same parsing path.
#[cfg(feature = "mitm")]
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
            "rewrite_local" | "rewrite_remote" => {
                parse_qx_rewrite(&mut cfg, &mut hook_index, line);
            }
            "task_local" => parse_qx_task(&mut cfg, dialect, line),
            "mitm" => parse_mitm_hostnames(&mut cfg, line),
            "script" => {
                // Loon `[Script]` lines start with `http-request|http-response`
                // (Surge uses `name = type=...`); determine type by first token to avoid
                // dialect confusion.
                let first = line.split_whitespace().next().unwrap_or_default();
                if matches!(first, "http-request" | "http-response") {
                    parse_loon_script(&mut cfg, line);
                } else {
                    parse_surge_script(&mut cfg, dialect, line);
                }
            }
            "argument" => parse_loon_argument(&mut cfg, line),
            "url rewrite" => parse_surge_url_rewrite(&mut cfg, line),
            "header rewrite" => parse_surge_header_rewrite(&mut cfg, line),
            "map local" => parse_surge_map_local(&mut cfg, line),
            // Other sections (e.g., QX `[task_remote]` / Surge `[General]`) are skipped entirely.
            _ => {}
        }
    }

    Ok(cfg)
}

/// `[mitm]` / `[MITM]` hostname line parsing: `hostname = a, b, -exclude`.
///
/// `-` / `!` prefixes are exclusions (normalized to `-`), kept in `cfg.hostnames`
/// alongside whitelist entries; downstream (`build_mitm_proxy` / core routing rules)
/// filters by prefix. Surge's `%APPEND%` prefix is stripped.
#[cfg(feature = "mitm")]
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
        if let Some(rest) = entry.strip_prefix('!') {
            cfg.hostnames.push(format!("-{rest}"));
        } else {
            cfg.hostnames.push(entry.to_string());
        }
    }
}
#[cfg(all(test, feature = "mitm"))]
mod tests;
