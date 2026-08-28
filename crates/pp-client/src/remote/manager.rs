//! Remote resource manager: manifest I/O, periodic fetching, and cache reading.

use std::path::PathBuf;

use pp_common::{PanelError, PanelResult};
use pp_script::ScriptDialect;

use crate::import::{ConfigMeta, ImportedConfig, parse_import};

use super::{
    FetchReport, ImportSummary, MergedRemoteConfig, RemoteKind,
    RemoteResource, safe_name,
};

/// Remote resource manager: responsible for remotes manifest I/O, periodic fetching, and cache reading.
#[derive(Debug, Clone)]
pub struct RemoteManager {
    data_dir: PathBuf,
}

impl RemoteManager {
    /// Create manager from data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    /// Remote manifest path: `data_dir/remotes.json`.
    pub fn remotes_file(&self) -> PathBuf {
        self.data_dir.join("remotes.json")
    }

    /// Read remote resource manifest; returns empty list when file does not exist.
    pub fn load(&self) -> PanelResult<Vec<RemoteResource>> {
        let path = self.remotes_file();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let text = std::fs::read_to_string(path)?;
        Ok(serde_json::from_str(&text)?)
    }

    /// Save remote resource manifest to `data_dir/remotes.json`.
    pub fn save(&self, remotes: &[RemoteResource]) -> PanelResult<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        let text = serde_json::to_string_pretty(remotes)?;
        std::fs::write(self.remotes_file(), text)?;
        Ok(())
    }

    /// Locate and fully update a remote resource by name (url / kind / dialect / update_interval_secs /
    /// enabled / description / argument_values / icon / arguments), keeping existing cache
    /// (`remote_cache/<name>.json` / `scripts/<name>.js` not deleted, overwritten on next fetch).
    /// Errors when resource does not exist.
    pub fn update_resource(&self, name: &str, updated: RemoteResource) -> PanelResult<()> {
        let mut remotes = self.load()?;
        let idx = remotes
            .iter()
            .position(|r| r.name == name)
            .ok_or_else(|| PanelError::Client(format!("远程资源 '{name}' 不存在")))?;
        // name is the lookup key, keep original name (do not rename on update).
        let mut entry = updated;
        entry.name = name.to_string();
        remotes[idx] = entry;
        self.save(&remotes)
    }

    /// Fetch all enabled remote resources and cache to disk.
    ///
    /// Single resource failures are only recorded in [`FetchReport::warnings`], not affecting other resources.
    pub async fn fetch_all(&self, remotes: &[RemoteResource]) -> FetchReport {
        let mut report = FetchReport::default();
        for remote in remotes.iter().filter(|r| r.enabled) {
            match remote.kind {
                RemoteKind::Script => match self.fetch_text(&remote.url).await {
                    Ok(text) => {
                        if let Err(e) = self.write_script(&remote.name, &text) {
                            report.warnings.push(format!(
                                "remote '{}': write script failed: {e}",
                                remote.name
                            ));
                            continue;
                        }
                        report.fetched += 1;
                        report.scripts += 1;
                    }
                    Err(e) => report
                        .warnings
                        .push(format!("remote '{}': {e}", remote.name)),
                },
                RemoteKind::Snippet => match self.fetch_snippet(remote).await {
                    Ok(imported) => {
                        report.rewrites += imported.rewrites.len();
                        report.scripts += imported.scripts.len();
                        report.tasks += imported.task_scripts.len();
                        for w in &imported.warnings {
                            report
                                .warnings
                                .push(format!("remote '{}': {w}", remote.name));
                        }
                        if let Err(e) = self.write_cache(&remote.name, &imported) {
                            report
                                .warnings
                                .push(format!("remote '{}': write cache failed: {e}", remote.name));
                            continue;
                        }
                        // Backfill arguments when resource declares none but remote meta has declarations.
                        if let Err(e) = self.backfill_arguments(&remote.name, &imported.meta) {
                            report.warnings.push(format!(
                                "remote '{}': backfill arguments failed: {e}",
                                remote.name
                            ));
                        }
                        report.fetched += 1;
                    }
                    Err(e) => report
                        .warnings
                        .push(format!("remote '{}': {e}", remote.name)),
                },
            }
            // Icon localization cache (best-effort): failure only recorded in warning, not affecting fetched count.
            if let Some(icon_url) = &remote.icon
                && let Err(e) = self.cache_icon(&remote.name, icon_url).await
            {
                report
                    .warnings
                    .push(format!("remote '{}': icon cache failed: {e}", remote.name));
            }
        }
        report
    }

    /// Read all Snippet caches and merge into runtime config.
    ///
    /// Single cache file corruption/missing only records warning and skips, not blocking other caches.
    pub fn load_cached(&self) -> PanelResult<MergedRemoteConfig> {
        let cache_dir = self.data_dir.join("remote_cache");
        let mut merged = MergedRemoteConfig::default();
        if !cache_dir.is_dir() {
            return Ok(merged);
        }
        let mut paths: Vec<PathBuf> = std::fs::read_dir(&cache_dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x == "json")
            })
            .collect();
        paths.sort();
        for path in paths {
            let text = match std::fs::read_to_string(&path) {
                Ok(t) => t,
                Err(e) => {
                    tracing::warn!(path = %path.display(), "read remote cache failed: {e}");
                    continue;
                }
            };
            let cached: super::CachedRemoteConfig = match serde_json::from_str(&text) {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), "parse remote cache failed: {e}");
                    continue;
                }
            };
            let part = cached.into_merged();
            merged.rewrites.extend(part.rewrites);
            merged.scripts.extend(part.scripts);
            merged.task_scripts.extend(part.task_scripts);
            merged.metas.extend(part.metas);
            for hostname in part.hostnames {
                if !merged.hostnames.contains(&hostname) {
                    merged.hostnames.push(hostname);
                }
            }
        }
        Ok(merged)
    }

    /// Merge an [`ImportedConfig`] into fixed cache `remote_cache/imported.json`.
    ///
    /// Script hooks / task scripts with empty `source` (not fetched) are skipped and counted in
    /// [`ImportSummary::warnings`]; rewrite rules and hostnames are directly merged.
    /// Repeated imports append on top of existing cache (do not overwrite).
    pub fn merge_imported(&self, imported: &ImportedConfig) -> PanelResult<ImportSummary> {
        let mut summary = ImportSummary::default();
        summary.warnings.extend(imported.warnings.iter().cloned());

        let cache_dir = self.data_dir.join("remote_cache");
        let cache_path = cache_dir.join("imported.json");
        let mut cached = super::CachedRemoteConfig::default();
        if cache_path.exists() {
            match std::fs::read_to_string(&cache_path) {
                Ok(text) => match serde_json::from_str::<super::CachedRemoteConfig>(&text) {
                    Ok(c) => cached = c,
                    Err(e) => summary.warnings.push(format!(
                        "existing local import cache unreadable, start fresh: {e}"
                    )),
                },
                Err(e) => summary.warnings.push(format!(
                    "existing local import cache unreadable, start fresh: {e}"
                )),
            }
        }

        for (rule, (name, url)) in imported.scripts.iter().zip(imported.script_urls.iter()) {
            if rule.source.is_empty() {
                summary.warnings.push(format!(
                    "script '{name}' source not fetched ({url}), skipped"
                ));
                continue;
            }
            cached.scripts.push(super::CachedScriptRule::from(rule));
            summary.scripts += 1;
        }
        for (task, url) in &imported.task_scripts {
            if task.source.is_empty() {
                summary.warnings.push(format!(
                    "task '{}' source not fetched ({url}), skipped",
                    task.name
                ));
                continue;
            }
            cached.task_scripts.push(task.clone());
            summary.tasks += 1;
        }
        cached
            .rewrites
            .extend(imported.rewrites.iter().map(super::CachedRewriteRule::from));
        summary.rewrites = imported.rewrites.len();
        for hostname in &imported.hostnames {
            if !cached.hostnames.contains(hostname) {
                cached.hostnames.push(hostname.clone());
            }
        }
        summary.hostnames = imported.hostnames.len();
        // Config header metadata (including `#!arguments` declarations) persisted with cache for runtime argument template replacement.
        if imported.meta != ConfigMeta::default() {
            cached.meta = Some(imported.meta.clone());
        }

        std::fs::create_dir_all(&cache_dir)?;
        let text = serde_json::to_string_pretty(&cached)?;
        std::fs::write(&cache_path, text)?;
        Ok(summary)
    }

    /// Import a third-party config content and merge into local cache:
    /// `parse_import` → [`Self::fill_script_sources`] (fetch script sources) → [`Self::merge_imported`].
    ///
    /// Single script / task script fetch failure only recorded in [`ImportSummary::warnings`] and skipped,
    /// not blocking rewrite rules and hostnames from being merged. Returns summary containing config header
    /// metadata and fetch statistics.
    pub async fn import_content(
        &self,
        content: &str,
        dialect: ScriptDialect,
    ) -> PanelResult<ImportSummary> {
        let mut imported = parse_import(content, dialect)
            .map_err(|e| PanelError::Client(format!("imported config parse failed: {e}")))?;
        self.fill_script_sources(&mut imported).await;
        let mut summary = self.merge_imported(&imported)?;
        summary.meta = imported.meta;
        Ok(summary)
    }

    /// Fetch single URL text content; non-2xx treated as failure.
    ///
    /// Delegates to [`crate::fetch_resource_text`]: GitHub blob/raw link normalization,
    /// GitHub proxy prefix拼接 and "use local proxy" settings are all handled uniformly (30s timeout).
    async fn fetch_text(&self, url: &str) -> PanelResult<String> {
        crate::fetch_resource_text(&self.data_dir, url, std::time::Duration::from_secs(30)).await
    }

    /// Backfill Snippet script hooks / task scripts' remote sources.
    ///
    /// Fetch scripts pointed to by [`ImportedConfig::script_urls`] and [`ImportedConfig::task_scripts`]
    /// one by one, write back to corresponding `source` on success; failure recorded in
    /// [`ImportedConfig::warnings`] and the script discarded, not blocking other scripts and rules.
    pub async fn fill_script_sources(&self, imported: &mut ImportedConfig) {
        let scripts = std::mem::take(&mut imported.scripts);
        let script_urls = std::mem::take(&mut imported.script_urls);
        let mut kept_scripts = Vec::new();
        let mut kept_urls = Vec::new();
        for (rule, (name, url)) in scripts.into_iter().zip(script_urls) {
            match self.fetch_text(&url).await {
                Ok(source) => {
                    let mut rule = rule;
                    rule.source = source;
                    kept_scripts.push(rule);
                    kept_urls.push((name, url));
                }
                Err(e) => imported
                    .warnings
                    .push(format!("hook script fetch failed: {e}")),
            }
        }
        // Keep scripts and script_urls aligned (only keep successfully fetched scripts)
        imported.scripts = kept_scripts;
        imported.script_urls = kept_urls;

        let task_scripts = std::mem::take(&mut imported.task_scripts);
        let mut kept_tasks = Vec::new();
        for (mut task, url) in task_scripts {
            match self.fetch_text(&url).await {
                Ok(source) => {
                    task.source = source;
                    kept_tasks.push((task, url));
                }
                Err(e) => imported
                    .warnings
                    .push(format!("task '{}' script fetch failed: {e}", task.name)),
            }
        }
        imported.task_scripts = kept_tasks;
    }

    /// Fetch backfill resource parameter declarations (auxiliary path): when manifest resource `arguments`
    /// is empty but remote meta declares parameters, backfill `arguments` and pre-fill `argument_values`
    /// with defaults (existing values in `argument_values` are not overwritten).
    fn backfill_arguments(&self, name: &str, meta: &ConfigMeta) -> PanelResult<()> {
        if meta.arguments.is_empty() {
            return Ok(());
        }
        let mut remotes = self.load()?;
        let Some(remote) = remotes.iter_mut().find(|r| r.name == name) else {
            // Resource has been deleted, skip backfill.
            return Ok(());
        };
        if !remote.arguments.is_empty() {
            // Already has parameter declarations (passed in when adding), do not overwrite.
            return Ok(());
        }
        for spec in &meta.arguments {
            if !remote.argument_values.iter().any(|(k, _)| k == &spec.key) {
                remote
                    .argument_values
                    .push((spec.key.clone(), spec.default_value.clone()));
            }
        }
        remote.arguments = meta.arguments.clone();
        self.save(&remotes)
    }

    /// Fetch Snippet: parse → backfill script hooks/task sources one by one (failure recorded in warning and skipped).
    async fn fetch_snippet(&self, remote: &RemoteResource) -> PanelResult<ImportedConfig> {
        let text = self.fetch_text(&remote.url).await?;
        let mut imported = parse_import(&text, remote.dialect).map_err(|e| {
            PanelError::Client(format!("snippet '{}' parse failed: {e}", remote.name))
        })?;
        self.fill_script_sources(&mut imported).await;
        Ok(imported)
    }

    /// Write pure JS script to `data_dir/scripts/<name>.js`.
    fn write_script(&self, name: &str, content: &str) -> PanelResult<PathBuf> {
        let dir = self.data_dir.join("scripts");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("{}.js", safe_name(name)));
        std::fs::write(&path, content)?;
        Ok(path)
    }

    /// Serialize Snippet aggregation result to cache `data_dir/remote_cache/<name>.json`.
    fn write_cache(&self, name: &str, imported: &ImportedConfig) -> PanelResult<PathBuf> {
        let dir = self.data_dir.join("remote_cache");
        std::fs::create_dir_all(&dir)?;
        let cached = super::CachedRemoteConfig::from_imported(imported);
        let text = serde_json::to_string_pretty(&cached)?;
        let path = dir.join(format!("{}.json", safe_name(name)));
        std::fs::write(&path, text)?;
        Ok(path)
    }

    /// Find `<safe_name(name)>.<ext>` icon cache under `data_dir/icons/`; return path when exists,
    /// otherwise `None`. ext must be in [`ICON_EXTENSIONS`] whitelist (case-insensitive).
    pub fn icon_file(&self, name: &str) -> Option<PathBuf> {
        let dir = self.data_dir.join("icons");
        let base = safe_name(name);
        for entry in std::fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.file_stem().and_then(|s| s.to_str()) != Some(base.as_str()) {
                continue;
            }
            let Some(ext) = path.extension().and_then(|x| x.to_str()) else {
                continue;
            };
            if super::ICON_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()) {
                return Some(path);
            }
        }
        None
    }

    /// Download remote resource icon to local cache `data_dir/icons/<safe_name>.<ext>`.
    ///
    /// Delegates to [`crate::fetch_resource_bytes`] (GitHub proxy prefix / use local proxy /
    /// 30s timeout consistent with text fetch). Extension priority: URL path suffix ([`ICON_EXTENSIONS`]
    /// whitelist) → infer image format from response bytes ([`icon_ext_from_bytes`]) → `.img`.
    /// Before writing, delete other old extension files for this name (no残留 after icon source/format change).
    ///
    /// Returns written path; returns `None` when response is empty (no icon content). Caller should
    /// best-effort handle return value, failure does not block main flow.
    pub async fn cache_icon(&self, name: &str, url: &str) -> PanelResult<Option<PathBuf>> {
        let bytes =
            crate::fetch_resource_bytes(&self.data_dir, url, std::time::Duration::from_secs(30))
                .await?;
        if bytes.is_empty() {
            return Ok(None);
        }
        let ext = super::icon_ext_from_url(url)
            .or_else(|| super::icon_ext_from_bytes(&bytes))
            .unwrap_or("img");

        let dir = self.data_dir.join("icons");
        std::fs::create_dir_all(&dir)?;
        let base = safe_name(name);
        // Delete other old extension files for this name (keep this write target).
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.file_stem().and_then(|s| s.to_str()) == Some(base.as_str())
                    && path.extension().and_then(|x| x.to_str()) != Some(ext)
                {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        let path = dir.join(format!("{base}.{ext}"));
        std::fs::write(&path, &bytes)?;
        Ok(Some(path))
    }
}
