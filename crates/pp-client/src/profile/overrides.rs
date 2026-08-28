//! Override-related functions: YAML deep-merge, JS override, remote override fetching and caching.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use pp_common::{PanelError, PanelResult};
use pp_script::{
    HttpExecutor, HttpRequestSpec, HttpResponseData, MemoryPersistentStore, ScriptDialect,
    ScriptHost, ScriptKind, ScriptLimits, ScriptWorker,
};
use serde_json::Value;

use crate::remote::TracingNotifier;

/// RFC 7386 style deep merge: when both target and patch are objects, recursively merge keys,
/// otherwise (array / scalar) replace entirely.
fn merge_deep(target: &mut Value, patch: &Value) {
    if let (Some(t), Some(p)) = (target.as_object_mut(), patch.as_object()) {
        for (key, patch_value) in p {
            match t.get_mut(key) {
                Some(target_value) => merge_deep(target_value, patch_value),
                None => {
                    t.insert(key.clone(), patch_value.clone());
                }
            }
        }
    } else {
        *target = patch.clone();
    }
}

/// YAML override: parse YAML then deep-merge into config per RFC 7386. Empty string / empty
/// document / null returns config unchanged.
///
/// Top level must be a mapping (object), otherwise errors.
pub fn apply_yaml_override(config: Value, yaml: &str) -> PanelResult<Value> {
    if yaml.trim().is_empty() {
        return Ok(config);
    }
    let patch: Value = serde_yaml::from_str(yaml)
        .map_err(|e| PanelError::Client(format!("invalid yaml override: {e}")))?;
    if patch.is_null() {
        return Ok(config);
    }
    if !patch.is_object() {
        return Err(PanelError::Client(
            "yaml override must be a YAML mapping".to_string(),
        ));
    }
    let mut merged = config;
    merge_deep(&mut merged, &patch);
    Ok(merged)
}

/// HttpExecutor that always denies all network requests: JS override environment has no
/// network permissions (deny implementation).
#[derive(Debug)]
pub struct DenyHttpExecutor;

#[async_trait]
impl HttpExecutor for DenyHttpExecutor {
    async fn execute(&self, req: HttpRequestSpec) -> PanelResult<HttpResponseData> {
        Err(PanelError::Client(format!(
            "network access denied in profile JS override: {}",
            req.url
        )))
    }
}

/// JS override: synchronous pure-function mode `function main(config){...; return config}`.
///
/// Wraps source with embedded config JSON (restored via `JSON.parse` to avoid object literal
/// `__proto__` traps), result passed back via `$done`; when `main` does not return
/// (undefined), original config is preserved. Empty source returns config unchanged.
///
/// Host: Surge dialect + [`DenyHttpExecutor`] (no network) + memory storage (no disk writes) +
/// [`TracingNotifier`]. Limits: 2 second timeout, default 32MB memory limit.
///
/// Executed via [`ScriptWorker`] (dedicated thread + current_thread runtime), returns `Send`
/// future; worker created per use (one thread spawn), thread naturally exits after job
/// completion when `tx` is dropped.
pub async fn apply_js_override(config: Value, js: &str) -> PanelResult<Value> {
    if js.trim().is_empty() {
        return Ok(config);
    }
    let cfg_json = serde_json::to_string(&config)?;
    let source = format!(
        "let __cfg = JSON.parse({cfg_lit});\n{js}\nlet __r = main(__cfg);\n$done(__r === undefined ? __cfg : __r);",
        cfg_lit = js_string_literal(&cfg_json)
    );
    let host = Arc::new(ScriptHost::new(
        Arc::new(DenyHttpExecutor),
        Arc::new(MemoryPersistentStore::new()),
        Arc::new(TracingNotifier::new()),
    ));
    let worker = ScriptWorker::new(
        host,
        ScriptLimits {
            timeout_ms: 2000,
            ..ScriptLimits::default()
        },
    );
    let out = worker
        .run_script(
            &source,
            ScriptKind::Generic,
            None,
            None,
            ScriptDialect::Surge,
            "profile-js",
        )
        .await?;
    Ok(out.0)
}

/// Resolve Profile remote override URLs: fetched at startup via shared fetch pipeline
/// [`crate::fetch_resource_text`] (URL normalization + GitHub proxy prefix + via local proxy +
/// retry + GitHub failure hint, 30 second timeout), successful fetch writes cache to
/// `profile_cache/<profile_id>.{yaml,js}`; failure falls back to cache; no cache → log warning
/// and skip that remote override (does not block startup).
///
/// Returns the overlaid effective overrides (remote as base, local overrides) and warning list;
/// [`ProfileOverrides`] is not produced at this layer, the "remote + local" merge for YAML/JS
/// is handled by [`super::build_core_config_v2`].
pub async fn resolve_remote_overrides(
    store_cache_dir: &Path,
    profile: &super::Profile,
) -> (super::EffectiveOverrides, Vec<String>) {
    let mut warnings = Vec::new();
    let key = profile.id.to_string();
    let remote_yaml = match profile.yaml_url.as_deref() {
        Some(url) if !url.trim().is_empty() => {
            fetch_remote_override(store_cache_dir, &key, url, "yaml", &mut warnings).await
        }
        _ => String::new(),
    };
    let remote_js = match profile.js_url.as_deref() {
        Some(url) if !url.trim().is_empty() => {
            fetch_remote_override(store_cache_dir, &key, url, "js", &mut warnings).await
        }
        _ => String::new(),
    };
    (
        super::EffectiveOverrides {
            remote_yaml,
            local_yaml: profile.yaml_override.clone(),
            remote_js,
            local_js: profile.js_override.clone(),
        },
        warnings,
    )
}

/// Fetch a single remote override: success writes cache; failure falls back to cache; both
/// failures log warning and return empty string.
async fn fetch_remote_override(
    cache_dir: &Path,
    key: &str,
    url: &str,
    ext: &str,
    warnings: &mut Vec<String>,
) -> String {
    match fetch_remote_text(cache_dir, url).await {
        Ok(text) => {
            if let Err(e) = write_override_cache(cache_dir, key, ext, &text) {
                warnings.push(format!("profile remote {ext} cache write failed: {e}"));
            }
            text
        }
        Err(e) => match read_override_cache(cache_dir, key, ext) {
            Ok(Some(text)) => {
                warnings.push(format!(
                    "profile remote {ext} fetch failed, fall back to cached: {e}"
                ));
                text
            }
            Ok(None) => {
                warnings.push(format!(
                    "profile remote {ext} fetch failed and no cached copy, skipped: {e}"
                ));
                String::new()
            }
            Err(read_err) => {
                warnings.push(format!(
                    "profile remote {ext} fetch failed and cached copy unreadable, \
                     skipped: {e}; cache: {read_err}"
                ));
                String::new()
            }
        },
    }
}

/// GET fetch remote override text: delegates to shared fetch pipeline
/// [`crate::fetch_resource_text`].
///
/// Shares the same pipeline with subscription / script / icon fetching: URL normalization +
/// GitHub proxy prefix + via local proxy + retry + GitHub failure hint, 30 second timeout.
/// Note UA difference: old implementation used fixed `clash.meta`, shared pipeline uses
/// reqwest default UA (if individual UA-sensitive subscription sources are affected, a custom
/// client can be restored here and [`crate::apply_github_proxy_prefix`] applied on top).
///
/// `data_dir` is derived from `cache_dir` (production callers pass `data_dir/profile_cache`)
/// by taking parent; in tests where cache_dir points directly to the data directory, parent
/// points to its parent directory, shared pipeline only best-effort reads `client.json`
/// (missing uses default settings), does not affect functionality.
async fn fetch_remote_text(cache_dir: &Path, url: &str) -> PanelResult<String> {
    let data_dir = cache_dir.parent().unwrap_or(cache_dir);
    crate::fetch_resource_text(data_dir, url, Duration::from_secs(30)).await
}

/// Remote override cache path: `<cache_dir>/<key>.<ext>` (key is profile id).
fn override_cache_path(cache_dir: &Path, key: &str, ext: &str) -> PathBuf {
    cache_dir.join(format!("{key}.{ext}"))
}

/// Write remote override cache (silent on success; returns Err for caller to log warning).
fn write_override_cache(cache_dir: &Path, key: &str, ext: &str, content: &str) -> PanelResult<()> {
    std::fs::create_dir_all(cache_dir)?;
    std::fs::write(override_cache_path(cache_dir, key, ext), content)?;
    Ok(())
}

/// Read remote override cache: returns `None` when missing.
fn read_override_cache(cache_dir: &Path, key: &str, ext: &str) -> PanelResult<Option<String>> {
    let path = override_cache_path(cache_dir, key, ext);
    if !path.exists() {
        return Ok(None);
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(e) => Err(PanelError::Client(format!(
            "read profile override cache failed: {e}"
        ))),
    }
}

/// Convert string to safe JS string literal (for embedding config JSON).
///
/// `serde_json` output already escapes control characters; here additionally handles quotes /
/// backslashes from JSON itself, and U+2028 / U+2029 which are not allowed in string literals
/// before QuickJS (ES2019).
fn js_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Compose remote + local two-segment JS override into chained source: remote `main` executes
/// first, local `main` executes second (local sees remote result). Each segment defines its
/// own `main` which would conflict, so IIFE is used to capture each and build a top-level
/// chained `main` (aligned with the `main(__cfg)` call at the end of [`apply_js_override`]'s
/// wrapper). Returns empty string only when both segments are empty (caller skips JS stage).
pub(crate) fn compose_js_chain(remote_js: &str, local_js: &str) -> String {
    let remote = remote_js.trim();
    let local = local_js.trim();
    if remote.is_empty() && local.is_empty() {
        return String::new();
    }
    let mut src = String::new();
    if !remote.is_empty() {
        src.push_str("let __r_main = (function() {\n");
        src.push_str(remote_js);
        src.push_str("\nreturn main;\n})();\n");
    }
    if !local.is_empty() {
        src.push_str("let __l_main = (function() {\n");
        src.push_str(local_js);
        src.push_str("\nreturn main;\n})();\n");
    }
    src.push_str("function main(__cfg) {\n");
    match (remote.is_empty(), local.is_empty()) {
        (false, false) => src.push_str("  return __l_main(__r_main(__cfg));\n"),
        (false, true) => src.push_str("  return __r_main(__cfg);\n"),
        (true, false) => src.push_str("  return __l_main(__cfg);\n"),
        (true, true) => {}
    }
    src.push_str("}\n");
    src
}
