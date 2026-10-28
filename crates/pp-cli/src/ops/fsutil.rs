//! Filesystem helpers: directory creation, file copy/move, backup management.

use anyhow::{Context, Result};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

use super::BACKUP_DIR;

/// Ensure a directory exists, creating it recursively if necessary.
pub async fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        tokio::fs::create_dir_all(path)
            .await
            .with_context(|| format!("failed to create dir {}", path.display()))?;
    }
    Ok(())
}

/// Find web assets in current or legacy Hub release archives.
pub fn find_web_dist(extract_dir: &Path) -> Option<PathBuf> {
    ["web/dist", "dist"]
        .into_iter()
        .map(|path| extract_dir.join(path))
        .find(|path| path.is_dir())
}

/// Write text to a file with optional permissions.
pub async fn write_file(path: &Path, contents: &str, mode: Option<u32>) -> Result<()> {
    let mut file = tokio::fs::File::create(path)
        .await
        .with_context(|| format!("failed to create {}", path.display()))?;
    file.write_all(contents.as_bytes())
        .await
        .with_context(|| format!("failed to write {}", path.display()))?;
    if let Some(m) = mode {
        let perms = std::fs::Permissions::from_mode(m);
        std::fs::set_permissions(path, perms)
            .with_context(|| format!("failed to chmod {}", path.display()))?;
    }
    Ok(())
}

/// Copy a file from src to dst.
pub async fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    tokio::fs::copy(src, dst)
        .await
        .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))
        .map(|_| ())
}

/// Remove a file if it exists.
pub async fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

/// Remove a directory only if it is empty.
pub async fn remove_dir_if_empty(path: &Path) -> Result<()> {
    if path.exists() {
        let mut entries = tokio::fs::read_dir(path).await?;
        if entries.next_entry().await?.is_none() {
            tokio::fs::remove_dir(path)
                .await
                .with_context(|| format!("failed to remove dir {}", path.display()))?;
        }
    }
    Ok(())
}

/// Move a file, falling back to copy+atomic-rename across filesystems (EXDEV).
/// The fallback copies to a sibling temp file first, then renames over the
/// destination: rename(2) is atomic and is allowed to replace a running
/// binary (unlike O_TRUNC writes, which fail with ETXTBSY).
pub async fn move_file(src: &Path, dst: &Path) -> Result<()> {
    match tokio::fs::rename(src, dst).await {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            let file_name = dst
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "bin".to_string());
            let staging = dst.with_file_name(format!(".{}", file_name));
            copy_file(src, &staging).await?;
            tokio::fs::rename(&staging, dst)
                .await
                .with_context(|| format!("failed to move to {}", dst.display()))?;
            tokio::fs::remove_file(src)
                .await
                .with_context(|| format!("failed to remove {}", src.display()))?;
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("failed to move to {}", dst.display())),
    }
}

/// Move a path (file or directory), handling cross-filesystem moves.
pub async fn move_path(src: &Path, dst: &Path) -> Result<()> {
    if !src.is_dir() {
        return move_file(src, dst).await;
    }
    match tokio::fs::rename(src, dst).await {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            copy_dir_recursive(src, dst).await?;
            tokio::fs::remove_dir_all(src)
                .await
                .with_context(|| format!("failed to remove {}", src.display()))?;
            Ok(())
        }
        Err(e) => Err(e).with_context(|| format!("failed to move to {}", dst.display())),
    }
}

/// Recursively copy a directory.
pub async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    ensure_dir(dst).await?;
    let mut entries = tokio::fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            Box::pin(copy_dir_recursive(&from, &to)).await?;
        } else {
            copy_file(&from, &to).await?;
        }
    }
    Ok(())
}

/// Generate a backup file path for a binary.
pub fn backup_path(binary_name: &str, version_or_timestamp: &str) -> PathBuf {
    backup_path_in(Path::new(BACKUP_DIR), binary_name, version_or_timestamp)
}

pub fn backup_path_in(backup_dir: &Path, name: &str, version_or_timestamp: &str) -> PathBuf {
    backup_dir.join(format!("{}.{}.bak", name, version_or_timestamp))
}

/// Find the most recent backup for a component binary or web directory.
pub async fn find_latest_backup_in(backup_dir: &Path, name: &str) -> Result<Option<PathBuf>> {
    let mut entries = tokio::fs::read_dir(backup_dir).await?;
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let entry_name = entry.file_name();
        let entry_name = entry_name.to_string_lossy();
        if entry_name.starts_with(&format!("{}.", name)) && entry_name.ends_with(".bak") {
            let meta = entry.metadata().await?;
            if let Ok(modified) = meta.modified() {
                candidates.push((entry.path(), modified));
            }
        }
    }
    candidates.sort_by_key(|backup| std::cmp::Reverse(backup.1));
    Ok(candidates.into_iter().next().map(|(path, _)| path))
}

/// Keep only the newest backup for a component binary (rollback needs one).
pub async fn prune_backups(bin_name: &str) {
    prune_backups_in(Path::new(BACKUP_DIR), bin_name).await;
}

pub async fn prune_backups_in(backup_dir: &Path, name: &str) {
    if !backup_dir.exists() {
        return;
    }
    let Ok(mut entries) = tokio::fs::read_dir(backup_dir).await else {
        return;
    };
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let entry_name = entry.file_name();
        let entry_name = entry_name.to_string_lossy();
        if entry_name.starts_with(&format!("{}.", name))
            && entry_name.ends_with(".bak")
            && let Ok(modified) = entry.metadata().await.and_then(|m| m.modified())
        {
            candidates.push((entry.path(), modified));
        }
    }
    candidates.sort_by_key(|backup| std::cmp::Reverse(backup.1));
    for (path, _) in candidates.into_iter().skip(1) {
        let result = if path.is_dir() {
            tokio::fs::remove_dir_all(&path).await
        } else {
            tokio::fs::remove_file(&path).await
        };
        let _ = result;
    }
}
