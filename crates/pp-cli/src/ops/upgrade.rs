//! Transactional upgrade and copy-based rollback commands.

use std::cmp::Ordering;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pp_common::release::{GitHubReleaseClient, compare_versions};
use tokio::process::Command;

use super::download::{download_and_verify, installed_version_strict};
use super::fsutil::{
    backup_path_in, copy_dir_recursive, copy_file, ensure_dir, find_latest_backup_in, move_file,
    move_path, prune_backups_in,
};
use super::maintenance::{MaintenanceLock, UpgradeOutcome, UpgradeStatus};
use super::systemd::{systemd_is_active, systemd_restart, systemd_stop, systemd_wait_stable};
use super::{BACKUP_DIR, BIN_DIR, OPT_DIR, release_arch, require_root};

#[derive(Clone, Copy)]
struct Component {
    name: &'static str,
    binary: &'static str,
    unit: Option<&'static str>,
    with_web: bool,
}

impl Component {
    fn parse(name: &str) -> Result<Self> {
        match name {
            "hub" => Ok(Self {
                name: "hub",
                binary: "proxy-panel-hub",
                unit: Some("proxy-panel-hub.service"),
                with_web: true,
            }),
            "agent" => Ok(Self {
                name: "agent",
                binary: "proxy-panel-agent",
                unit: Some("proxy-panel-agent.service"),
                with_web: false,
            }),
            "cli" => Ok(Self {
                name: "cli",
                binary: "proxy-panel",
                unit: None,
                with_web: false,
            }),
            other => bail!("未知组件: {}", other),
        }
    }
}

#[derive(Debug)]
pub(super) struct UpgradePaths {
    pub bin_dir: PathBuf,
    pub opt_dir: PathBuf,
    pub backup_dir: PathBuf,
}

impl UpgradePaths {
    fn system() -> Self {
        Self {
            bin_dir: PathBuf::from(BIN_DIR),
            opt_dir: PathBuf::from(OPT_DIR),
            backup_dir: PathBuf::from(BACKUP_DIR),
        }
    }

    fn binary(&self, component: Component) -> PathBuf {
        self.bin_dir.join(component.binary)
    }

    fn web_dist(&self) -> PathBuf {
        self.opt_dir.join("web/dist")
    }
}

#[derive(Debug, PartialEq, Eq)]
enum UpgradeDecision {
    Proceed,
    Skip,
}

fn plan_upgrade(current: Option<&str>, target: &str) -> Result<UpgradeDecision> {
    let Some(current) = current else {
        return Ok(UpgradeDecision::Proceed);
    };
    match compare_versions(current, target)? {
        Ordering::Equal => Ok(UpgradeDecision::Skip),
        Ordering::Greater => bail!(
            "目标版本 {} 低于当前版本 {}；降级请使用 rollback",
            target,
            current
        ),
        Ordering::Less => Ok(UpgradeDecision::Proceed),
    }
}

/// Upgrade a component (agent, hub, or cli), optionally reporting JSON status.
pub async fn upgrade(
    component: &str,
    version: &str,
    repo: &str,
    status_file: Option<PathBuf>,
) -> Result<()> {
    require_root()?;
    let component = Component::parse(component)?;
    let _lock = MaintenanceLock::acquire(Path::new(OPT_DIR))?;
    let mut status = UpgradeStatus::new(status_file, component.name, Some(version.to_string()));
    let mut rolled_back = false;
    let result = perform_upgrade(
        component,
        version,
        repo,
        &UpgradePaths::system(),
        &mut status,
        &mut rolled_back,
    )
    .await;
    match result {
        Ok(()) => {
            status.finish(UpgradeOutcome::Succeeded, "升级完成").await?;
            println!("{} 升级完成。", component.name);
            Ok(())
        }
        Err(error) => {
            let outcome = if rolled_back {
                UpgradeOutcome::RolledBack
            } else {
                UpgradeOutcome::Failed
            };
            if let Err(status_error) = status.finish(outcome, &error.to_string()).await {
                eprintln!("写入升级状态失败: {status_error:#}");
            }
            Err(error)
        }
    }
}

async fn perform_upgrade(
    component: Component,
    requested_version: &str,
    repo: &str,
    paths: &UpgradePaths,
    status: &mut UpgradeStatus,
    rolled_back: &mut bool,
) -> Result<()> {
    status.stage("resolving", "正在解析目标 Release").await?;
    let releases = GitHubReleaseClient::new(repo)?;
    let release = if requested_version == "latest" {
        releases.latest_stable().await?
    } else {
        releases.by_tag(requested_version).await?
    };
    let target_version = release.version.to_string();
    status.set_target_version(target_version.clone());

    let binary_path = paths.binary(component);
    let current_version = installed_version_strict(&binary_path).await?;
    status.set_current_version(current_version.clone());
    if plan_upgrade(current_version.as_deref(), &target_version)? == UpgradeDecision::Skip {
        status
            .stage("skipped", "当前版本已是目标版本，跳过升级")
            .await?;
        println!(
            "{} 当前版本 {} 已是目标版本，跳过升级。",
            component.name, target_version
        );
        return Ok(());
    }

    status
        .stage("preparing", "正在下载、校验并解压 Release")
        .await?;
    let arch = release_arch()?;
    let asset = format!("proxy-panel-{}-linux-{}.tar.gz", component.name, arch);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .context("failed to build HTTP client")?;
    let temp = tempfile::tempdir().context("failed to create temp dir")?;
    let archive = download_and_verify(&client, repo, &release.tag, &asset, temp.path()).await?;
    let extract_dir = temp.path().join("extract");
    ensure_dir(&extract_dir).await?;
    extract_tar_gz(&archive, &extract_dir).await?;
    let new_binary = find_extracted_binary(&extract_dir, component.binary).await?;
    let new_web = extract_dir.join("web/dist");
    if component.with_web && !new_web.is_dir() {
        bail!("hub tarball 中未找到 web/dist");
    }

    ensure_dir(&paths.backup_dir).await?;
    let backup_label =
        current_version.unwrap_or_else(|| chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
    let binary_backup = backup_existing(
        &binary_path,
        &backup_path_in(&paths.backup_dir, component.binary, &backup_label),
    )
    .await?;
    let web_path = paths.web_dist();
    let web_backup = if component.with_web {
        backup_existing(
            &web_path,
            &backup_path_in(&paths.backup_dir, "web-dist", &backup_label),
        )
        .await?
    } else {
        None
    };

    let was_active = match component.unit {
        Some(unit) => systemd_is_active(unit).await,
        None => false,
    };
    if let (Some(unit), true) = (component.unit, was_active) {
        systemd_stop(unit).await?;
    }

    status
        .stage("replacing", "正在替换文件并验证目标版本")
        .await?;
    let result = replace_and_verify(
        component,
        &new_binary,
        &new_web,
        &binary_path,
        &web_path,
        &target_version,
        was_active,
    )
    .await;
    if let Err(error) = result {
        if let Err(status_error) = status
            .stage("rollback", "升级失败，正在恢复原文件和服务")
            .await
        {
            eprintln!("写入回滚状态失败: {status_error:#}");
        }
        match recover(
            component,
            &binary_backup,
            &web_backup,
            &binary_path,
            &web_path,
            was_active,
        )
        .await
        {
            Ok(()) => {
                *rolled_back = true;
                bail!("升级失败，已恢复旧版本: {error:#}");
            }
            Err(recovery_error) => {
                bail!("升级失败: {error:#}；自动恢复也失败: {recovery_error:#}");
            }
        }
    }

    prune_backups_in(&paths.backup_dir, component.binary).await;
    if component.with_web {
        prune_backups_in(&paths.backup_dir, "web-dist").await;
    }
    status
        .stage("completed", "目标版本和服务状态验证通过")
        .await?;
    Ok(())
}

async fn backup_existing(source: &Path, backup: &Path) -> Result<Option<PathBuf>> {
    if !source.exists() {
        return Ok(None);
    }
    if source.is_dir() {
        copy_dir_recursive(source, backup).await?;
    } else {
        copy_file(source, backup).await?;
    }
    Ok(Some(backup.to_path_buf()))
}

#[allow(clippy::too_many_arguments)]
async fn replace_and_verify(
    component: Component,
    new_binary: &Path,
    new_web: &Path,
    binary_path: &Path,
    web_path: &Path,
    target_version: &str,
    was_active: bool,
) -> Result<()> {
    move_file(new_binary, binary_path).await?;
    std::fs::set_permissions(binary_path, std::fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to chmod {}", binary_path.display()))?;
    if component.with_web {
        remove_path(web_path).await?;
        ensure_dir(web_path.parent().unwrap_or(Path::new("/"))).await?;
        move_path(new_web, web_path).await?;
        let _ = Command::new("chown")
            .arg("-R")
            .arg(format!("{}:{}", super::USER_NAME, super::USER_NAME))
            .arg(web_path)
            .status()
            .await;
        println!("web 控制台已更新");
    }

    let actual = installed_version_strict(binary_path)
        .await?
        .with_context(|| "升级后二进制不存在")?;
    if compare_versions(&actual, target_version)? != Ordering::Equal {
        bail!("目标版本验证失败: 期望 {}，实际 {}", target_version, actual);
    }
    if let (Some(unit), true) = (component.unit, was_active) {
        systemd_restart(unit).await?;
        systemd_wait_stable(unit).await?;
    }
    Ok(())
}

async fn recover(
    component: Component,
    binary_backup: &Option<PathBuf>,
    web_backup: &Option<PathBuf>,
    binary_path: &Path,
    web_path: &Path,
    was_active: bool,
) -> Result<()> {
    let mut errors = Vec::new();
    if let Err(error) = restore_path(binary_backup.as_deref(), binary_path).await {
        errors.push(format!("恢复二进制失败: {error:#}"));
    }
    if component.with_web
        && let Err(error) = restore_path(web_backup.as_deref(), web_path).await
    {
        errors.push(format!("恢复 web/dist 失败: {error:#}"));
    }
    if let (Some(unit), true) = (component.unit, was_active) {
        if let Err(error) = systemd_restart(unit).await {
            errors.push(format!("恢复原服务失败: {error:#}"));
        } else if let Err(error) = systemd_wait_stable(unit).await {
            errors.push(format!("原服务未恢复稳定: {error:#}"));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        bail!(errors.join("；"))
    }
}

/// Restore from a copy, retaining the backup for subsequent rollback attempts.
pub(super) async fn restore_path(backup: Option<&Path>, destination: &Path) -> Result<()> {
    remove_path(destination).await?;
    let Some(backup) = backup else {
        return Ok(());
    };
    if backup.is_dir() {
        copy_dir_recursive(backup, destination).await?;
    } else {
        copy_file(backup, destination).await?;
    }
    Ok(())
}

async fn remove_path(path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    if path.is_dir() {
        tokio::fs::remove_dir_all(path).await?;
    } else {
        tokio::fs::remove_file(path).await?;
    }
    Ok(())
}

async fn find_extracted_binary(extract_dir: &Path, binary_name: &str) -> Result<PathBuf> {
    let mut entries = tokio::fs::read_dir(extract_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_name().to_string_lossy() == binary_name {
            return Ok(entry.path());
        }
    }
    bail!("tarball 中未找到 {}", binary_name)
}

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

/// Roll back Hub/Agent, including the Hub web console, without consuming backups.
pub async fn rollback(component: &str) -> Result<()> {
    require_root()?;
    let component = Component::parse(component)?;
    if component.name == "cli" {
        bail!("未知组件: cli；rollback 仅支持 agent 或 hub");
    }
    let _lock = MaintenanceLock::acquire(Path::new(OPT_DIR))?;
    let paths = UpgradePaths::system();
    let binary_backup = find_latest_backup_in(&paths.backup_dir, component.binary)
        .await?
        .with_context(|| format!("未找到 {} 的备份文件", component.binary))?;
    let web_backup = if component.with_web {
        Some(
            find_latest_backup_in(&paths.backup_dir, "web-dist")
                .await?
                .with_context(|| "未找到 Hub web/dist 备份")?,
        )
    } else {
        None
    };
    let binary_path = paths.binary(component);
    let was_active = match component.unit {
        Some(unit) => systemd_is_active(unit).await,
        None => false,
    };
    if let (Some(unit), true) = (component.unit, was_active) {
        systemd_stop(unit).await?;
    }
    restore_path(Some(&binary_backup), &binary_path).await?;
    if component.with_web {
        restore_path(web_backup.as_deref(), &paths.web_dist()).await?;
    }
    if let (Some(unit), true) = (component.unit, was_active) {
        systemd_restart(unit).await?;
        systemd_wait_stable(unit).await?;
    }
    println!("{} 已回滚。", component.name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_version_is_skipped_and_downgrade_is_rejected() {
        assert_eq!(
            plan_upgrade(Some("1.2.3"), "v1.2.3").unwrap(),
            UpgradeDecision::Skip
        );
        let error = plan_upgrade(Some("1.3.0"), "1.2.3").unwrap_err();
        assert!(error.to_string().contains("rollback"));
        assert_eq!(
            plan_upgrade(Some("1.2.3"), "1.3.0").unwrap(),
            UpgradeDecision::Proceed
        );
    }
}
