//! Upgrade and rollback commands.

use anyhow::{Context, Result, bail};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::process::Command;

use super::{BACKUP_DIR, BIN_DIR, OPT_DIR, require_root, release_arch};
use super::download::{download_and_verify, installed_version};
use super::fsutil::{
    backup_path, copy_file, ensure_dir, move_file, move_path, prune_backups,
};
use super::systemd::{
    systemd_is_active, systemd_restart, systemd_stop,
};

/// Upgrade a component (agent, hub, or cli).
pub async fn upgrade(component: &str, version: &str, repo: &str) -> Result<()> {
    require_root()?;
    let arch = release_arch()?;

    match component {
        "hub" => {
            let asset = format!("proxy-panel-hub-linux-{}.tar.gz", arch);
            upgrade_component("proxy-panel-hub", version, repo, &asset, true, true).await?;
        }
        "agent" => {
            let asset = format!("proxy-panel-agent-linux-{}.tar.gz", arch);
            upgrade_component("proxy-panel-agent", version, repo, &asset, true, false).await?;
        }
        "cli" => {
            let asset = format!("proxy-panel-cli-linux-{}.tar.gz", arch);
            upgrade_component("proxy-panel", version, repo, &asset, false, false).await?;
        }
        other => bail!("未知组件: {}", other),
    }

    println!("{} 升级完成。", component);
    Ok(())
}

async fn upgrade_component(
    bin_name: &str,
    version: &str,
    repo: &str,
    asset: &str,
    has_service: bool,
    with_web_dist: bool,
) -> Result<()> {
    let bin_path = PathBuf::from(BIN_DIR).join(bin_name);
    let unit = format!(
        "proxy-panel-{}.service",
        bin_name.trim_start_matches("proxy-panel-")
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .context("failed to build HTTP client")?;

    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let archive = download_and_verify(&client, repo, version, asset, tmp_dir.path()).await?;

    ensure_dir(Path::new(BACKUP_DIR)).await?;

    // Stop the service before replacing the binary (running binaries are
    // busy and cannot be overwritten in place).
    let was_active = has_service && systemd_is_active(&unit).await;
    if was_active {
        systemd_stop(&unit).await?;
    }

    // Backup current binary
    let current_ver = installed_version(&bin_path)
        .await?
        .unwrap_or_else(|| chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
    let bak = backup_path(bin_name, &current_ver);
    if bin_path.exists() {
        copy_file(&bin_path, &bak).await?;
    }

    // Extract and replace binary
    let extract_dir = tmp_dir.path().join("extract");
    ensure_dir(&extract_dir).await?;
    extract_tar_gz(&archive, &extract_dir).await?;

    let mut new_bin: Option<PathBuf> = None;
    let mut entries = tokio::fs::read_dir(&extract_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        // For hub tarball there may be multiple files; pick the exact binary name
        if name_str == bin_name {
            new_bin = Some(entry.path());
        }
        // For cli tarball the root file is "proxy-panel"
        if bin_name == "proxy-panel" && name_str == "proxy-panel" {
            new_bin = Some(entry.path());
        }
    }

    let src = new_bin.with_context(|| format!("tarball 中未找到 {}", bin_name))?;
    move_file(&src, &bin_path).await?;
    let perms = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(&bin_path, perms)
        .with_context(|| format!("failed to chmod {}", bin_path.display()))?;

    // Hub tarballs also carry the web console; swap it in alongside the binary.
    if with_web_dist {
        let new_dist = extract_dir.join("web").join("dist");
        if new_dist.is_dir() {
            let web_dst = PathBuf::from(OPT_DIR).join("web/dist");
            if web_dst.exists() {
                tokio::fs::remove_dir_all(&web_dst)
                    .await
                    .with_context(|| format!("failed to remove old {}", web_dst.display()))?;
            }
            ensure_dir(Path::new(OPT_DIR).join("web").as_path()).await?;
            move_path(&new_dist, &web_dst).await?;
            let _ = Command::new("chown")
                .arg("-R")
                .arg(format!("{}:{}", super::USER_NAME, super::USER_NAME))
                .arg(OPT_DIR)
                .status()
                .await;
            println!("web 控制台已更新");
        }
    }

    if was_active {
        systemd_restart(&unit).await?;
        if !systemd_is_active(&unit).await {
            // Rollback
            eprintln!("服务启动失败，正在自动回滚...");
            if bak.exists() {
                move_file(&bak, &bin_path).await?;
                systemd_restart(&unit).await?;
                if systemd_is_active(&unit).await {
                    println!("已回滚到旧版本并恢复运行。");
                    bail!("升级失败，已自动回滚到旧版本");
                } else {
                    bail!("升级失败，回滚后服务仍无法启动，请手动检查");
                }
            }
            bail!("升级失败且无可用备份，无法回滚");
        }
    }

    // Upgrade succeeded: keep only the newest backup (previous version).
    prune_backups(bin_name).await;

    Ok(())
}

/// Tar extraction helper (spawns system tar).
async fn extract_tar_gz(archive: &Path, dest_dir: &Path) -> Result<()> {
    let status = Command::new("tar")
        .args([
            "-xzf",
            &archive.to_string_lossy(),
            "-C",
            &dest_dir.to_string_lossy(),
        ])
        .status()
        .await
        .context("failed to run tar")?;
    if !status.success() {
        bail!("tar 解压失败: {}", archive.display());
    }
    Ok(())
}

/// Rollback a component to its latest backup.
pub async fn rollback(component: &str) -> Result<()> {
    require_root()?;
    let (bin_name, unit) = match component {
        "hub" => ("proxy-panel-hub", "proxy-panel-hub.service"),
        "agent" => ("proxy-panel-agent", "proxy-panel-agent.service"),
        other => bail!("未知组件: {}", other),
    };

    let bak = super::fsutil::find_latest_backup(bin_name)
        .await?
        .with_context(|| format!("未找到 {} 的备份文件", bin_name))?;

    let bin_path = PathBuf::from(BIN_DIR).join(bin_name);
    super::systemd::systemd_stop(unit).await?;
    move_file(&bak, &bin_path).await?;

    super::systemd::systemd_restart(unit).await?;
    if !super::systemd::systemd_is_active(unit).await {
        bail!("回滚后 {} 仍无法启动", unit);
    }

    println!("{} 已回滚并重启成功。", component);
    Ok(())
}
