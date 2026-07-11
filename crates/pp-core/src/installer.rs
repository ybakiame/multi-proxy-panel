//! Automatic core binary downloader / installer.
//!
//! On first use, the agent can fetch the appropriate xray/sing-box binary from
//! GitHub releases and extract it into the configured binary directory.

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
    match core_type {
        CoreType::Xray => ReleaseInfo {
            owner: "XTLS",
            repo: "Xray-core",
            binary_name: "xray",
        },
        CoreType::SingBox => ReleaseInfo {
            owner: "SagerNet",
            repo: "sing-box",
            binary_name: "sing-box",
        },
        CoreType::Both => ReleaseInfo {
            owner: "",
            repo: "",
            binary_name: "",
        },
    }
}

fn env_version(core_type: CoreType) -> Option<String> {
    let key = match core_type {
        CoreType::Xray => "PROXYPANEL_XRAY_VERSION",
        CoreType::SingBox => "PROXYPANEL_SINGBOX_VERSION",
        CoreType::Both => return None,
    };
    std::env::var(key).ok().filter(|s| !s.is_empty())
}

fn target_info() -> PanelResult<(&'static str, &'static str, &'static str, bool)> {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;

    let (xray_arch, sb_arch, is_windows) = match (os, arch) {
        ("linux", "x86_64") => ("linux-64", "linux-amd64", false),
        ("linux", "aarch64") => ("linux-arm64", "linux-arm64", false),
        ("macos", "x86_64") => ("macos-64", "darwin-amd64", false),
        ("macos", "aarch64") => ("macos-arm64", "darwin-arm64", false),
        ("windows", "x86_64") => ("windows-64", "windows-amd64", true),
        _ => {
            return Err(PanelError::Core(format!(
                "unsupported platform for auto-install: {}-{}",
                os, arch
            )));
        }
    };

    Ok((os, xray_arch, sb_arch, is_windows))
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
    let (_, xray_arch, sb_arch, is_windows) = target_info()?;

    // Asset names usually omit the leading 'v' present in GitHub tags.
    let version_no_v = version.strip_prefix('v').unwrap_or(version);

    match core_type {
        CoreType::Xray => Ok((format!("Xray-{}.zip", xray_arch), "xray".to_string())),
        CoreType::SingBox => {
            let ext = if is_windows { "zip" } else { "tar.gz" };
            Ok((
                format!("sing-box-{}-{}.{}", version_no_v, sb_arch, ext),
                "sing-box".to_string(),
            ))
        }
        CoreType::Both => Err(PanelError::Core(
            "Cannot auto-install 'Both' core type".into(),
        )),
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
            return Ok(dest);
        }
    }

    Err(PanelError::Core(format!(
        "binary {} not found in archive {}",
        target_name, archive_display
    )))
}

fn core_type_from_name(name: &str) -> PanelResult<CoreType> {
    match name {
        "xray" => Ok(CoreType::Xray),
        "sing-box" => Ok(CoreType::SingBox),
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

/// Ensure the requested core binary exists in `bin_dir`, downloading it from
/// GitHub releases if necessary. `version` overrides the default (latest/env)
/// when provided and is non-empty.
pub async fn ensure_core_binary(
    bin_dir: &Path,
    core_type: CoreType,
    version: Option<&str>,
) -> PanelResult<PathBuf> {
    if core_type == CoreType::Both {
        return Err(PanelError::Core("Cannot install 'Both' core type".into()));
    }

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
    download_file(&url, &tmp_archive).await?;

    let dest = tokio::task::block_in_place(move || {
        let result = if asset.ends_with(".tar.gz") {
            extract_tgz(&tmp_archive, bin_dir, &binary_inside)
        } else if asset.ends_with(".zip") {
            extract_zip(&tmp_archive, bin_dir, &binary_inside)
        } else {
            Err(PanelError::Core(format!(
                "unknown archive format for asset {}",
                asset
            )))
        };
        // Best-effort cleanup of the archive
        let _ = std::fs::remove_file(&tmp_archive);
        result
    });

    let path = dest?;
    set_executable(&path)?;

    tracing::info!("installed {:?} binary at {}", core_type, path.display());
    Ok(path)
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
        for (ct, version) in [(CoreType::Xray, "v25.6.8"), (CoreType::SingBox, "v1.11.0")] {
            let (asset, _) = asset_name(ct, version).unwrap();
            assert!(asset.ends_with(".zip") || asset.ends_with(".tar.gz"));
        }
    }
}
