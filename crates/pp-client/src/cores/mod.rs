//! Core version management: download local cores + detect system-installed cores + active selection.
//!
//! # Reuse conclusion
//!
//! Does not directly reuse `pp-core::installer`'s `ensure_core_binary`:
//!
//! - Its semantics are for agent's "single-directory disk + environment variable/latest version
//!   resolution", which differs from this module's "`data_dir/cores/<core>/<version>/` version
//!   directory + explicit version";
//! - Its GitHub domain is fixed, cannot inject mock services for local acceptance testing;
//! - This module retains its asset naming and extraction ideas, implementing a simplified version
//!   for the client scenario.
//!
//! Remote versions and assets are always based on GitHub Release API: real asset naming may deviate
//! from convention (e.g. mihomo Alpha channel uses short commit hash naming), so during download
//! the release's `assets` list is matched by platform / architecture first, rather than directly
//! constructing the download URL.

use std::path::{Path, PathBuf};

use pp_common::{CoreType, PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::config::ClientConfig;

mod download;
#[cfg(test)]
mod tests;
mod version;

/// GitHub API request timeout (seconds).
const HTTP_TIMEOUT_SECS: u64 = 30;

/// Core source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreSource {
    /// Downloaded from GitHub Releases to `data_dir/cores` by this module.
    Downloaded,
    /// System-installed core detected via PATH.
    System,
}

/// A locally available core (downloaded or system-installed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCore {
    pub core_type: CoreType,
    pub version: String,
    pub path: PathBuf,
    pub source: CoreSource,
}

/// Client core inventory: manages download directory scanning, remote version listing,
/// download installation, system detection, and active matching.
#[derive(Debug, Clone)]
pub struct ClientCoreInventory {
    data_dir: PathBuf,
    api_base: String,
    client: reqwest::Client,
}

impl ClientCoreInventory {
    /// Create inventory based on data directory (GitHub API uses `https://api.github.com`).
    pub fn new(data_dir: PathBuf) -> Self {
        Self::with_api_base(data_dir, "https://api.github.com")
    }

    /// Specify GitHub API base (for injecting mock service addresses in tests).
    pub fn with_api_base(data_dir: PathBuf, api_base: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(HTTP_TIMEOUT_SECS))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self {
            data_dir,
            api_base: api_base.into(),
            client,
        }
    }

    /// GitHub proxy prefix (from settings page "GitHub Access"): best-effort read from
    /// `data_dir/client.json`, falls back to empty string (direct connection) on missing/corrupt file.
    ///
    /// Core version queries and downloads share the same GitHub access strategy (URL prefix
    /// concatenation) with remote resource fetching; the user-configured proxy prefix applies to
    /// both `api.github.com` release queries and `github.com` release asset downloads.
    fn github_proxy_prefix(&self) -> String {
        ClientConfig::load(&self.data_dir)
            .unwrap_or_default()
            .github_proxy_prefix
    }

    /// Download directory: `data_dir/cores`.
    pub fn cores_dir(&self) -> PathBuf {
        self.data_dir.join("cores")
    }

    /// Specific core version directory: `data_dir/cores/<core>/<version>`.
    fn core_dir(&self, core_type: CoreType, version: &str) -> PathBuf {
        self.cores_dir()
            .join(version::binary_name(core_type))
            .join(version)
    }

    /// Scan `cores_dir/<type>/<version>/` and list downloaded cores.
    pub fn list_installed(&self) -> Vec<LocalCore> {
        let mut out = Vec::new();
        for core_type in [CoreType::SingBox, CoreType::Mihomo] {
            let type_dir = self.cores_dir().join(version::binary_name(core_type));
            let Ok(entries) = std::fs::read_dir(&type_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let version = entry.file_name().to_string_lossy().into_owned();
                let bin = path.join(version::binary_name_on_disk(core_type));
                if bin.is_file() {
                    out.push(LocalCore {
                        core_type,
                        version,
                        path: bin,
                        source: CoreSource::Downloaded,
                    });
                }
            }
        }
        out.sort_by(|a, b| {
            a.core_type
                .to_string()
                .cmp(&b.core_type.to_string())
                .then_with(|| a.version.cmp(&b.version))
                .then_with(|| a.path.cmp(&b.path))
        });
        out
    }

    /// Scan `cores_dir/<type>/<version>/` version directories, list downloaded version numbers
    /// for this core type, sorted by semantic version descending (latest first, prerelease lower
    /// than same-base stable).
    pub fn list_downloaded_versions(&self, core_type: CoreType) -> Vec<String> {
        let mut versions: Vec<String> = self
            .list_installed()
            .into_iter()
            .filter(|c| c.core_type == core_type && c.source == CoreSource::Downloaded)
            .map(|c| c.version)
            .collect();
        versions.sort_by(|a, b| version::compare_core_versions(b, a));
        versions
    }

    /// List recent 10 remote release versions (strip `v` prefix).
    pub async fn list_remote_versions(&self, core_type: CoreType) -> PanelResult<Vec<String>> {
        let (owner, repo) = core_type.github_repo();
        let url = format!(
            "{}/repos/{}/{}/releases?per_page=10",
            self.api_base, owner, repo
        );
        // GitHub API URL is wrapped by configured proxy prefix (shares GitHub access strategy
        // with remote resource fetching); injected mock service addresses (non-GitHub domains)
        // are not affected.
        let url = crate::apply_github_proxy_prefix(&url, &self.github_proxy_prefix());
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("GitHub API request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "GitHub API returned status {}",
                resp.status()
            )));
        }
        let releases: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| PanelError::Core(format!("Failed to parse GitHub releases: {e}")))?;
        let mut versions = Vec::new();
        for release in releases {
            if let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) {
                let version = tag.strip_prefix('v').unwrap_or(tag);
                if !version.is_empty() {
                    versions.push(version.to_string());
                }
            }
        }
        Ok(versions)
    }

    /// Download specified core version and save to `cores_dir/<type>/<version>/`.
    ///
    /// Reuses existing download if same version already present; after download extracts,
    /// chmod 755, and verifies version probe output (`version` / `--version` / `-v` tried in
    /// sequence) contains target version.
    pub async fn download(&self, core_type: CoreType, version: &str) -> PanelResult<LocalCore> {
        let version = version.strip_prefix('v').unwrap_or(version).to_string();
        let tag = version::github_tag(&version);
        let (arch_hint, is_windows) = version::target_spec()?;

        let dir = self.core_dir(core_type, &version);
        let on_disk = dir.join(version::binary_name_on_disk(core_type));

        // Reuse if already downloaded and version verification passes.
        if on_disk.is_file() && version::verify_version(&on_disk, core_type, &version).is_ok() {
            return Ok(LocalCore {
                core_type,
                version,
                path: on_disk,
                source: CoreSource::Downloaded,
            });
        }

        tokio::fs::create_dir_all(&dir).await?;

        // Match current platform asset via GitHub Release API (real naming may deviate from
        // convention, do not construct URL directly).
        let (asset_url, asset_name) = self
            .resolve_asset_url(core_type, &tag, arch_hint, is_windows)
            .await
            .map_err(|e| {
                PanelError::Core(format!(
                    "Failed to resolve {} {} release asset: {e}",
                    core_type, tag
                ))
            })?;
        // Asset download initial URL is `github.com/<owner>/<repo>/releases/download/...`,
        // wrapped by configured proxy prefix; 302 redirect to `objects.githubusercontent.com`
        // is followed by the gh proxy side (not wrapped again here).
        let asset_url = crate::apply_github_proxy_prefix(&asset_url, &self.github_proxy_prefix());
        tracing::info!(
            core_type = %core_type,
            version = %version,
            url = %asset_url,
            "Downloading core"
        );

        let tmp_archive = dir.join(format!(".download-{asset_name}"));
        self.download_to(&asset_url, &tmp_archive).await?;

        let binary_inside = version::binary_name(core_type);
        let (dir_clone, archive_clone) = (dir.clone(), tmp_archive.clone());
        let result = tokio::task::spawn_blocking(move || {
            if asset_name.ends_with(".tar.gz") {
                download::extract_tgz(&archive_clone, &dir_clone, binary_inside)
            } else if asset_name.ends_with(".zip") {
                download::extract_zip(&archive_clone, &dir_clone, binary_inside)
            } else if asset_name.ends_with(".gz") {
                download::extract_gzip(&archive_clone, &dir_clone, binary_inside)
            } else {
                Err(PanelError::Core(format!(
                    "Unknown asset format: {asset_name}"
                )))
            }
        })
        .await
        .map_err(|e| PanelError::Core(format!("Extraction task failed: {e}")))??;
        // Best-effort cleanup of download archive.
        let _ = std::fs::remove_file(&tmp_archive);

        let path = result;
        download::set_executable(&path)?;

        // Version probe verification: output must contain target version; on failure clean up
        // directory to avoid leaving partial artifacts.
        if let Err(e) = version::verify_version(&path, core_type, &version) {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }

        tracing::info!(
            core_type = %core_type,
            version = %version,
            path = %path.display(),
            "Core download complete"
        );
        Ok(LocalCore {
            core_type,
            version,
            path,
            source: CoreSource::Downloaded,
        })
    }

    /// Look up system-installed cores via PATH (`sing-box` / `mihomo`, append `.exe` on Windows),
    /// try `version` / `--version` / `-v` in sequence and parse version number; on parse failure
    /// record as `unknown`.
    pub fn detect_system_cores(&self) -> Vec<LocalCore> {
        let Some(path_value) = std::env::var_os("PATH") else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for dir in std::env::split_paths(&path_value) {
            if !dir.is_dir() {
                continue;
            }
            for core_type in [CoreType::SingBox, CoreType::Mihomo] {
                let candidate = dir.join(version::binary_name_on_disk(core_type));
                if !candidate.is_file() || out.iter().any(|c: &LocalCore| c.path == candidate) {
                    continue;
                }
                let version = version::parse_version_from_output(
                    core_type,
                    &version::binary_output(&candidate),
                )
                .unwrap_or_else(|| "unknown".to_string());
                out.push(LocalCore {
                    core_type,
                    version,
                    path: candidate,
                    source: CoreSource::System,
                });
            }
        }
        out
    }

    /// Match installed / system core by `config.core_binary`; returns `None` when not set.
    pub fn active_core(&self, config: &ClientConfig) -> Option<LocalCore> {
        if config.core_binary.as_os_str().is_empty() {
            return None;
        }
        self.list_installed()
            .into_iter()
            .chain(self.detect_system_cores())
            .find(|c| paths_equal(&c.path, &config.core_binary))
    }

    /// Preferred local binary for a core type:
    ///
    /// 1. The highest version among downloaded cores (semantic version sorting, prerelease lower
    ///    than same-base stable, e.g. `1.14.0-beta.4` < `1.14.0` but `> 1.13.15`);
    /// 2. Fallback to first system core of this type detected in PATH when no downloaded cores;
    /// 3. Neither → `None` (command layer prompts user to download from core management).
    pub fn preferred_binary(&self, core_type: CoreType) -> Option<PathBuf> {
        let downloaded = self
            .list_installed()
            .into_iter()
            .filter(|c| c.core_type == core_type)
            .max_by(|a, b| version::compare_core_versions(&a.version, &b.version));
        if let Some(core) = downloaded {
            return Some(core.path);
        }
        self.detect_system_cores()
            .into_iter()
            .find(|c| c.core_type == core_type)
            .map(|c| c.path)
    }

    /// Delete a downloaded core (only cores within `cores_dir`).
    ///
    /// Deletes the entire `cores/<type>/<version>/` version directory; cleans up type directory
    /// if empty after deletion.
    /// Errors: path outside `cores_dir` (system core) / path does not exist / core is
    /// `active_binary` (in use).
    ///
    /// Safety constraint: `canonicalize` path ownership check before actual deletion, prevents
    /// deleting anything outside `cores_dir` (directory traversal / symlink escape protection).
    pub fn delete(&self, path: &Path, active_binary: &Path) -> PanelResult<()> {
        // Anti-traversal: after canonicalize confirm target is inside download directory.
        // If download directory does not exist, there are no downloaded cores to delete.
        let cores_dir = std::fs::canonicalize(self.cores_dir())
            .map_err(|_| PanelError::Core("Core download directory does not exist".to_string()))?;
        let bin = std::fs::canonicalize(path)
            .map_err(|e| PanelError::Core(format!("Core binary does not exist: {e}")))?;
        if !bin.starts_with(&cores_dir) {
            return Err(PanelError::Core(
                "System core cannot be deleted: only cores in download directory are supported"
                    .to_string(),
            ));
        }
        // Structure validation: target must be of form `cores/<type>/<version>/<binary>`,
        // avoid accidentally deleting type directory or entire download directory.
        if !bin.is_file() {
            return Err(PanelError::Core("Invalid core binary path".to_string()));
        }
        let version_dir = bin
            .parent()
            .ok_or_else(|| PanelError::Core("Cannot locate core version directory".to_string()))?;
        let type_dir = version_dir
            .parent()
            .ok_or_else(|| PanelError::Core("Cannot locate core type directory".to_string()))?;
        if type_dir.parent() != Some(cores_dir.as_path()) {
            return Err(PanelError::Core("Invalid core binary path".to_string()));
        }
        // Core in use cannot be deleted.
        if paths_equal(&bin, active_binary) {
            return Err(PanelError::Core(
                "Active core cannot be deleted: please switch to another core first".to_string(),
            ));
        }
        tracing::info!(
            path = %bin.display(),
            version_dir = %version_dir.display(),
            "Deleting local core"
        );
        std::fs::remove_dir_all(version_dir)
            .map_err(|e| PanelError::Core(format!("Failed to delete core: {e}")))?;
        // Clean up type directory (`cores/<type>/`) if empty.
        if std::fs::read_dir(type_dir)
            .map(|mut it| it.next().is_none())
            .unwrap_or(false)
        {
            let _ = std::fs::remove_dir(type_dir);
        }
        Ok(())
    }

    /// Download `url` to `dest` (chunked streaming to disk).
    async fn download_to(&self, url: &str, dest: &Path) -> PanelResult<()> {
        let mut resp = self
            .client
            .get(url)
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("Download request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "Download returned status {} ({})",
                resp.status(),
                url
            )));
        }
        let mut file = tokio::fs::File::create(dest).await?;
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| PanelError::Core(format!("Download stream error: {e}")))?
        {
            file.write_all(&chunk).await?;
        }
        Ok(())
    }

    /// Match asset from GitHub release `assets` by platform / architecture, returns
    /// (download URL, asset name).
    async fn resolve_asset_url(
        &self,
        core_type: CoreType,
        tag: &str,
        arch_hint: &str,
        is_windows: bool,
    ) -> PanelResult<(String, String)> {
        let (owner, repo) = core_type.github_repo();
        let url = format!(
            "{}/repos/{}/{}/releases/tags/{}",
            self.api_base, owner, repo, tag
        );
        // GitHub API URL is wrapped by configured proxy prefix (shares GitHub access strategy
        // with remote resource fetching); injected mock service addresses (non-GitHub domains)
        // are not affected.
        let url = crate::apply_github_proxy_prefix(&url, &self.github_proxy_prefix());
        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("GitHub release request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "GitHub release returned status {}",
                resp.status()
            )));
        }
        let release: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| PanelError::Core(format!("Failed to parse GitHub release: {e}")))?;
        let assets = release
            .get("assets")
            .and_then(|v| v.as_array())
            .ok_or_else(|| PanelError::Core("release has no assets field".to_string()))?;

        for asset in assets {
            let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.contains(arch_hint)
                && version::ext_ok(core_type, is_windows, name)
                && let Some(url) = asset.get("browser_download_url").and_then(|v| v.as_str())
            {
                return Ok((url.to_string(), name.to_string()));
            }
        }
        Err(PanelError::Core(format!(
            "No asset found for platform {} in release {tag}",
            arch_hint
        )))
    }
}

/// Lenient path equality: direct comparison, or compare after resolving real paths.
fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}
