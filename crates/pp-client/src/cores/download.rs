//! Download-related private methods and free functions for core management.

use std::path::{Path, PathBuf};

use pp_common::{PanelError, PanelResult};

use super::version::{binary_name_on_disk, core_type_from_name};

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
            let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
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
            let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
            let mut out = std::fs::File::create(&dest)?;
            std::io::copy(&mut entry, &mut out)?;
            binary_dest = Some(dest);
        }
    }
    binary_dest
        .ok_or_else(|| PanelError::Core(format!("Binary {target_name} not found in archive")))
}

/// Extract single gzip file (mihomo non-Windows asset is a single binary gz).
pub(super) fn extract_gzip(
    archive: &Path,
    dest_dir: &Path,
    target_name: &str,
) -> PanelResult<PathBuf> {
    let file = std::fs::File::open(archive)?;
    let mut decoder = flate2::read::GzDecoder::new(file);
    let dest = dest_dir.join(binary_name_on_disk(core_type_from_name(target_name)?));
    let mut out = std::fs::File::create(&dest)?;
    std::io::copy(&mut decoder, &mut out)?;
    Ok(dest)
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
    Ok(())
}
