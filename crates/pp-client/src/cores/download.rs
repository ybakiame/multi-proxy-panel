//! Download-related private methods and free functions for core management.

use std::path::{Path, PathBuf};

use pp_common::{PanelError, PanelResult};

use super::version::binary_name_on_disk;

/// Extract `.tar.gz` and retrieve target binary.
pub(super) fn extract_tgz(
    archive: &Path,
    dest_dir: &Path,
    target_name: &str,
) -> PanelResult<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let tar = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(tar);
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let file_name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if file_name == target_name || file_name == format!("{target_name}.exe") {
            let dest = dest_dir.join(binary_name_on_disk());
            entry.unpack(&dest)?;
            return Ok(dest);
        }
    }
    Err(PanelError::Core(format!(
        "Binary {target_name} not found in archive"
    )))
}

/// Extract `.zip` and retrieve target binary.
pub(super) fn extract_zip(
    archive: &Path,
    dest_dir: &Path,
    target_name: &str,
) -> PanelResult<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| PanelError::Core(format!("Invalid zip archive: {e}")))?;
    let mut binary_dest: Option<PathBuf> = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PanelError::Core(format!("Zip entry error: {e}")))?;
        let file_name = std::path::Path::new(entry.name())
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        if file_name.eq_ignore_ascii_case(target_name)
            || file_name.eq_ignore_ascii_case(&format!("{target_name}.exe"))
        {
            let dest = dest_dir.join(binary_name_on_disk());
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
            binary_dest = Some(dest);
        }
    }
    binary_dest
        .ok_or_else(|| PanelError::Core(format!("Binary {target_name} not found in archive")))
}

/// chmod 755 (Unix).
pub(super) fn set_executable(path: &Path) -> PanelResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(path)?.permissions();
        perms.set_mode(perms.mode() | 0o755);
        std::fs::set_permissions(path, perms)?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Extract a single file from a zip archive by suffix path match (e.g. `bin/amd64/wintun.dll`)
/// to `dest`.
#[cfg(target_os = "windows")]
pub(super) fn extract_zip_entry(archive: &Path, suffix: &str, dest: &Path) -> PanelResult<()> {
    let file = std::fs::File::open(archive)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| PanelError::Core(format!("Invalid zip archive: {e}")))?;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| PanelError::Core(format!("Zip entry error: {e}")))?;
        // zip 内路径统一为正斜杠；按后缀匹配避免依赖顶层目录名（wintun-x.y.z/ 前缀随版本变）。
        let name = entry.name().replace('\\', "/");
        if name.ends_with(suffix) {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut out)?;
            return Ok(());
        }
    }
    Err(PanelError::Core(format!(
        "Entry {suffix} not found in archive"
    )))
}

/// Windows：确保核心目录内存在 `wintun.dll`（sing-box TUN 依赖，核心不内嵌）。
///
/// 已存在则跳过；否则从 wintun.net 官方发布下载 zip（版本固定，避免最新版链接漂移），
/// 按当前架构提取 `bin/<arch>/wintun.dll` 落到核心目录。
#[cfg(target_os = "windows")]
pub(super) async fn ensure_wintun(client: &reqwest::Client, core_dir: &Path) -> PanelResult<()> {
    const WINTUN_VERSION: &str = "0.14.1";
    let dest = core_dir.join("wintun.dll");
    if dest.is_file() {
        return Ok(());
    }
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        other => {
            return Err(PanelError::Core(format!(
                "Unsupported Windows arch for wintun: {other}"
            )));
        }
    };
    let url = format!("https://www.wintun.net/builds/wintun-{WINTUN_VERSION}.zip");
    let tmp = core_dir.join(".wintun-download.zip");
    download_to(client, &url, &tmp).await?;
    let tmp_clone = tmp.clone();
    let dest_clone = dest.clone();
    let suffix = format!("bin/{arch}/wintun.dll");
    let result =
        tokio::task::spawn_blocking(move || extract_zip_entry(&tmp_clone, &suffix, &dest_clone))
            .await
            .map_err(|e| PanelError::Core(format!("wintun extraction task failed: {e}")))?;
    let _ = std::fs::remove_file(&tmp);
    result
}

/// Download `url` to `dest`（分块流式落盘；供 ensure_wintun 复用，独立于
/// `ClientCoreInventory::download_to` 以保持本模块自包含）。
#[cfg(target_os = "windows")]
async fn download_to(client: &reqwest::Client, url: &str, dest: &Path) -> PanelResult<()> {
    use tokio::io::AsyncWriteExt;

    let mut resp = client
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
