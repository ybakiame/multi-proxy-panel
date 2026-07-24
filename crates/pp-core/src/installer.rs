//! Automatic core binary downloader / installer.
//!
//! On first use, the agent can fetch the appropriate sing-box/mihomo binary
//! from GitHub releases and extract it into the configured binary directory.

use pp_common::{CoreType, PanelError, PanelResult};
use serde_json::Value;
use std::path::{Path, PathBuf};

const GITHUB_API_TIMEOUT_SECS: u64 = 10;
const DOWNLOAD_TIMEOUT_SECS: u64 = 120;

struct ReleaseInfo {
    owner: &'static str,
    repo: &'static str,
    binary_name: &'static str,
}

fn release_info(core_type: CoreType) -> ReleaseInfo {
    let (owner, repo) = core_type.github_repo();
    let binary_name = match core_type {
        CoreType::SingBox => "sing-box",
        CoreType::Mihomo => "mihomo",
    };
    ReleaseInfo {
        owner,
        repo,
        binary_name,
    }
}

fn env_version(core_type: CoreType) -> Option<String> {
    let key = match core_type {
        CoreType::SingBox => "PROXYPANEL_SINGBOX_VERSION",
        CoreType::Mihomo => "PROXYPANEL_MIHOMO_VERSION",
    };
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

fn target_info() -> PanelResult<(&'static str, &'static str, bool)> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let (sb_arch, is_windows) = match (os, arch) {
        ("linux", "x86_64") => ("linux-amd64", false),
        ("linux", "aarch64") => ("linux-arm64", false),
        ("macos", "x86_64") => ("darwin-amd64", false),
        ("macos", "aarch64") => ("darwin-arm64", false),
        ("windows", "x86_64") => ("windows-amd64", true),
        _ => {
            return Err(PanelError::Core(format!(
                "unsupported platform for auto-install: {}-{}",
                os, arch
            )));
        }
    };

    Ok((os, sb_arch, is_windows))
}

/// GitHub release tags include a leading 'v', but callers may pass either
/// `1.14.0-alpha.43` or `v1.14.0-alpha.43`. This helper normalizes to the
/// tag name used in release URLs.
fn github_tag(version: &str) -> String {
    if version.starts_with('v') {
        version.to_string()
    } else {
        format!("v{version}")
    }
}

fn asset_name(core_type: CoreType, version: &str) -> PanelResult<(String, String)> {
    let (_, sb_arch, is_windows) = target_info()?;

    // Asset names usually omit the leading 'v' present in GitHub tags.
    let version_no_v = version.strip_prefix('v').unwrap_or(version);

    match core_type {
        CoreType::SingBox => {
            let ext = if is_windows { "zip" } else { "tar.gz" };
            Ok((
                format!("sing-box-{}-{}.{}", version_no_v, sb_arch, ext),
                "sing-box".to_string(),
            ))
        }
        CoreType::Mihomo => {
            // mihomo ships a single gzipped binary (zip on Windows), and the
            // asset name keeps the tag's leading 'v'.
            let ext = if is_windows { "zip" } else { "gz" };
            Ok((
                format!("mihomo-{}-{}.{}", sb_arch, github_tag(version), ext),
                "mihomo".to_string(),
            ))
        }
    }
}

fn binary_name_on_disk(core_type: CoreType) -> String {
    let base = release_info(core_type).binary_name;
    if std::env::consts::OS == "windows" {
        format!("{}.exe", base)
    } else {
        base.to_string()
    }
}

/// On-disk path of a core's binary inside `bin_dir`.
pub fn core_binary_path(bin_dir: &Path, core_type: CoreType) -> PathBuf {
    bin_dir.join(binary_name_on_disk(core_type))
}

async fn fetch_latest_version(owner: &str, repo: &str) -> PanelResult<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(GITHUB_API_TIMEOUT_SECS))
        .build()
        .map_err(|e| PanelError::Core(format!("failed to build http client: {}", e)))?;

    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );
    let resp = client
        .get(&url)
        .header("User-Agent", "proxy-panel-agent")
        .send()
        .await
        .map_err(|e| PanelError::Core(format!("GitHub API request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(PanelError::Core(format!(
            "GitHub API returned status {}",
            resp.status()
        )));
    }

    let value: Value = resp
        .json()
        .await
        .map_err(|e| PanelError::Core(format!("failed to parse GitHub release: {}", e)))?;

    value
        .get("tag_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| PanelError::Core("GitHub release has no tag_name".into()))
}

async fn resolve_version(core_type: CoreType) -> PanelResult<String> {
    if let Some(v) = env_version(core_type) {
        return Ok(v);
    }

    let info = release_info(core_type);
    let version = fetch_latest_version(info.owner, info.repo).await?;
    tracing::info!("resolved latest version for {:?}: {}", core_type, version);
    Ok(version)
}

async fn download_file(url: &str, dest: &Path) -> PanelResult<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .build()
        .map_err(|e| PanelError::Core(format!("failed to build http client: {}", e)))?;

    let mut resp = client
        .get(url)
        .header("User-Agent", "proxy-panel-agent")
        .send()
        .await
        .map_err(|e| PanelError::Core(format!("download request failed: {}", e)))?;

    if !resp.status().is_success() {
        return Err(PanelError::Core(format!(
            "download returned status {} for {}",
            resp.status(),
            url
        )));
    }

    let mut file = tokio::fs::File::create(dest).await?;
    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| PanelError::Core(format!("download stream error: {}", e)))?
    {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
    }

    Ok(())
}

fn extract_tgz(archive: &Path, dest_dir: &Path, target_name: &str) -> PanelResult<PathBuf> {
    let archive_display = archive.display().to_string();
    let file = std::fs::File::open(archive)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if file_name == target_name || file_name == format!("{}.{}", target_name, "exe") {
            let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
            entry.unpack(&dest)?;
            return Ok(dest);
        }
    }

    Err(PanelError::Core(format!(
        "binary {} not found in archive {}",
        target_name, archive_display
    )))
}

fn extract_zip(archive: &Path, dest_dir: &Path, target_name: &str) -> PanelResult<PathBuf> {
    let archive_display = archive.display().to_string();
    let file = std::fs::File::open(archive)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| PanelError::Core(format!("invalid zip archive: {}", e)))?;

    let mut binary_dest: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PanelError::Core(format!("zip entry error: {}", e)))?;
        let path = entry.name();
        let file_name = std::path::Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if file_name.eq_ignore_ascii_case(target_name)
            || file_name.eq_ignore_ascii_case(&format!("{}.{}", target_name, "exe"))
        {
            let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
            binary_dest = Some(dest);
        }
    }

    match binary_dest {
        Some(dest) => Ok(dest),
        None => Err(PanelError::Core(format!(
            "binary {} not found in archive {}",
            target_name, archive_display
        ))),
    }
}

fn extract_gzip(archive: &Path, dest_dir: &Path, target_name: &str) -> PanelResult<PathBuf> {
    use std::io::Read;

    let file = std::fs::File::open(archive)?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
    let mut out = std::fs::File::create(&dest)?;
    let mut buf = Vec::new();
    decoder.read_to_end(&mut buf)?;
    std::io::Write::write_all(&mut out, &buf)?;
    Ok(dest)
}

fn core_type_from_name(name: &str) -> PanelResult<CoreType> {
    match name {
        "sing-box" => Ok(CoreType::SingBox),
        "mihomo" => Ok(CoreType::Mihomo),
        _ => Err(PanelError::Core(format!(
            "unknown core binary name: {}",
            name
        ))),
    }
}

fn set_executable(path: &Path) -> PanelResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms)?;
    }
    Ok(())
}

/// Download `url` to `dest`; on failure, resolve the real asset through the
/// GitHub release API and retry. Rolling tags (e.g. mihomo Prerelease-Alpha)
/// name assets with a short commit hash that cannot be derived from the tag,
/// so the conventional direct URL 404s.
async fn download_with_fallback(
    url: &str,
    core_type: CoreType,
    version: &str,
    asset: &str,
    dest: &Path,
) -> PanelResult<()> {
    match download_file(url, dest).await {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::debug!("direct asset download failed ({}), resolving via API", e);
            let api_url = resolve_asset_url(core_type, version, asset).await?;
            download_file(&api_url, dest).await
        }
    }
}

/// Find the browser-download URL of the asset matching this platform in the
/// GitHub release for `version`.
async fn resolve_asset_url(core_type: CoreType, version: &str, asset: &str) -> PanelResult<String> {
    let info = release_info(core_type);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(GITHUB_API_TIMEOUT_SECS))
        .build()
        .map_err(|e| PanelError::Core(format!("failed to build http client: {}", e)))?;

    // Try the tag both with and without the conventional 'v' prefix.
    let bare = version.strip_prefix('v').unwrap_or(version);
    for tag in [github_tag(version), bare.to_string()] {
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/tags/{}",
            info.owner, info.repo, tag
        );
        let resp = match client
            .get(&url)
            .header("User-Agent", "proxy-panel-agent")
            .send()
            .await
        {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };
        let release: Value = resp
            .json()
            .await
            .map_err(|e| PanelError::Core(format!("failed to parse release {}: {}", tag, e)))?;

        // Match assets by platform/arch substrings taken from the
        // conventionally-named asset (e.g. "linux-amd64" out of
        // "mihomo-linux-amd64-v1.19.29.gz").
        let arch_hint = asset_arch_hint(core_type, asset);
        let ext_ok = |name: &str| {
            (asset.ends_with(".tar.gz") && name.ends_with(".tar.gz"))
                || (asset.ends_with(".zip") && name.ends_with(".zip"))
                || (asset.ends_with(".gz")
                    && !asset.ends_with(".tar.gz")
                    && name.ends_with(".gz")
                    && !name.ends_with(".tar.gz"))
        };
        if let Some(assets) = release.get("assets").and_then(|v| v.as_array()) {
            for a in assets {
                let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
                if name.contains(&arch_hint) && ext_ok(name) {
                    if let Some(url) = a.get("browser_download_url").and_then(|v| v.as_str()) {
                        return Ok(url.to_string());
                    }
                }
            }
        }
    }

    Err(PanelError::Core(format!(
        "no matching asset found for {:?} {}",
        core_type, version
    )))
}

/// Extract the `os-arch` hint from a conventionally named asset.
fn asset_arch_hint(_core_type: CoreType, _asset: &str) -> String {
    let (_, sb_arch, _) = target_info().unwrap_or(("linux", "linux-amd64", false));
    sb_arch.to_string()
}

/// Ensure the requested core binary exists in `bin_dir`, downloading it from
/// GitHub releases if necessary. `version` overrides the default (latest/env)
/// when provided and is non-empty.
pub async fn ensure_core_binary(
    bin_dir: &Path,
    core_type: CoreType,
    version: Option<&str>,
) -> PanelResult<PathBuf> {
    let on_disk = bin_dir.join(binary_name_on_disk(core_type));
    if tokio::fs::try_exists(&on_disk).await.unwrap_or(false) {
        return Ok(on_disk);
    }

    let version = if let Some(v) = version {
        if v.is_empty() {
            resolve_version(core_type).await?
        } else {
            v.to_string()
        }
    } else {
        resolve_version(core_type).await?
    };
    let (asset, binary_inside) = asset_name(core_type, &version)?;
    let info = release_info(core_type);
    let url = format!(
        "https://github.com/{}/{}/releases/download/{}/{}",
        info.owner,
        info.repo,
        github_tag(&version),
        asset
    );

    tracing::info!("downloading {:?} {} from {}", core_type, version, url);

    tokio::fs::create_dir_all(bin_dir).await?;
    let tmp_archive = bin_dir.join(format!(".{}", asset));

    // Retry the download once: truncated or otherwise corrupted archives
    // (e.g. from a flaky network) are detected by validating the payload
    // before extraction.
    let mut last_err: Option<PanelError> = None;
    let mut dest: Option<PathBuf> = None;
    for attempt in 1..=2 {
        match download_with_fallback(&url, core_type, &version, &asset, &tmp_archive).await {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!("download failed (attempt {}): {}", attempt, e);
                last_err = Some(e);
                continue;
            }
        }
        if let Err(e) = validate_archive(&tmp_archive, &asset) {
            tracing::warn!(
                "downloaded archive failed validation (attempt {}): {}",
                attempt,
                e
            );
            last_err = Some(e);
            continue;
        }

        let result = tokio::task::block_in_place(|| {
            if asset.ends_with(".tar.gz") {
                extract_tgz(&tmp_archive, bin_dir, &binary_inside)
            } else if asset.ends_with(".zip") {
                extract_zip(&tmp_archive, bin_dir, &binary_inside)
            } else if asset.ends_with(".gz") {
                extract_gzip(&tmp_archive, bin_dir, &binary_inside)
            } else {
                Err(PanelError::Core(format!(
                    "unknown archive format for asset {}",
                    asset
                )))
            }
        });

        match result {
            Ok(path) => {
                dest = Some(path);
                break;
            }
            Err(e) => {
                tracing::warn!("archive extraction failed (attempt {}): {}", attempt, e);
                last_err = Some(e);
            }
        }
    }
    // Best-effort cleanup of the archive
    let _ = tokio::fs::remove_file(&tmp_archive).await;

    let path = match dest {
        Some(p) => p,
        None => {
            return Err(last_err.unwrap_or_else(|| {
                PanelError::Core(format!("failed to install {:?} from {}", core_type, url))
            }));
        }
    };
    set_executable(&path)?;

    tracing::info!("installed {:?} binary at {}", core_type, path.display());
    Ok(path)
}

/// Sanity-check a downloaded archive before extraction: it must be larger
/// than any plausible error page and carry the expected magic bytes.
fn validate_archive(archive: &Path, asset: &str) -> PanelResult<()> {
    let meta = std::fs::metadata(archive)?;
    if meta.len() < 1024 * 1024 {
        let preview = std::fs::read(archive)
            .ok()
            .map(|b| String::from_utf8_lossy(&b[..b.len().min(200)]).to_string())
            .unwrap_or_default();
        return Err(PanelError::Core(format!(
            "downloaded archive {} is suspiciously small ({} bytes): {}",
            asset,
            meta.len(),
            preview
        )));
    }

    let mut magic = [0u8; 4];
    {
        use std::io::Read;
        let mut f = std::fs::File::open(archive)?;
        f.read_exact(&mut magic)
            .map_err(|e| PanelError::Core(format!("failed to read archive magic: {}", e)))?;
    }
    let ok = if asset.ends_with(".zip") {
        magic.starts_with(b"PK")
    } else {
        // .tar.gz and .gz both start with the gzip magic
        magic.starts_with(b"\x1f\x8b")
    };
    if !ok {
        return Err(PanelError::Core(format!(
            "downloaded archive {} has unexpected magic bytes {:02x?}",
            asset, magic
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_tag_normalizes_version_strings() {
        assert_eq!(github_tag("v1.14.0-alpha.43"), "v1.14.0-alpha.43");
        assert_eq!(github_tag("1.14.0-alpha.43"), "v1.14.0-alpha.43");
        assert_eq!(github_tag("v25.6.8"), "v25.6.8");
        assert_eq!(github_tag("25.6.8"), "v25.6.8");
    }

    #[test]
    fn asset_names_are_well_formed() {
        for (ct, version) in [
            (CoreType::SingBox, "v1.11.0"),
            (CoreType::Mihomo, "v1.19.28"),
        ] {
            let (asset, _) = asset_name(ct, version).unwrap();
            assert!(asset.ends_with(".zip") || asset.ends_with(".gz"));
        }
    }
}
