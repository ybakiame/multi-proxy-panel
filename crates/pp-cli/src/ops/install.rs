//! Install commands for Hub and Agent components.

use anyhow::{Context, Result, bail};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

use super::{
    BACKUP_DIR, BIN_DIR, ETC_DIR, OPT_DIR, USER_NAME, VAR_DIR, AGENT_UNIT, HUB_UNIT,
    require_root, release_arch,
};
use super::download::{download_and_verify, installed_version};
use super::fsutil::{
    backup_path, copy_file, ensure_dir, move_file, move_path, prune_backups, write_file,
};
use super::systemd::{
    systemd_daemon_reload, systemd_enable_now, systemd_is_active, systemd_restart, systemd_stop,
};

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

/// Ensure the proxypanel system user exists.
async fn ensure_user() -> Result<()> {
    let status = Command::new("id")
        .arg(USER_NAME)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("failed to run id")?;
    if !status.success() {
        let out = Command::new("useradd")
            .args(["-r", "-s", "/bin/false", USER_NAME])
            .output()
            .await
            .context("failed to run useradd")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            bail!("创建用户 {} 失败: {}", USER_NAME, err);
        }
    }
    Ok(())
}

/// Install the Hub component.
pub async fn install_hub(version: &str, repo: &str) -> Result<()> {
    require_root()?;
    let arch = release_arch()?;
    let asset = format!("proxy-panel-hub-linux-{}.tar.gz", arch);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .context("failed to build HTTP client")?;

    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let archive = download_and_verify(&client, repo, version, &asset, tmp_dir.path()).await?;

    // Ensure directories
    ensure_dir(Path::new(BIN_DIR)).await?;
    ensure_dir(Path::new(ETC_DIR)).await?;
    ensure_dir(Path::new(OPT_DIR)).await?;
    ensure_dir(Path::new(&format!("{}/web/dist", OPT_DIR))).await?;
    ensure_dir(Path::new(BACKUP_DIR)).await?;
    ensure_user().await?;

    // Extract to temp first, then move binaries
    let extract_dir = tmp_dir.path().join("extract");
    ensure_dir(&extract_dir).await?;
    extract_tar_gz(&archive, &extract_dir).await?;

    // Find extracted binaries and web/dist
    let mut hub_bin_src: Option<PathBuf> = None;
    let mut cli_bin_src: Option<PathBuf> = None;
    let mut web_dist_src: Option<PathBuf> = None;

    let mut entries = tokio::fs::read_dir(&extract_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == "proxy-panel-hub" {
            hub_bin_src = Some(entry.path());
        } else if name_str == "proxy-panel" {
            cli_bin_src = Some(entry.path());
        } else if name_str == "web" {
            // look for dist inside
            let dist = entry.path().join("dist");
            if dist.is_dir() {
                web_dist_src = Some(dist);
            }
        }
    }

    let hub_dst = PathBuf::from(BIN_DIR).join("proxy-panel-hub");
    let cli_dst = PathBuf::from(BIN_DIR).join("proxy-panel");
    let web_dst = PathBuf::from(OPT_DIR).join("web/dist");

    // Stop the running service before replacing its binary (ETXTBSY).
    let hub_was_active = systemd_is_active("proxy-panel-hub").await;
    if hub_was_active {
        systemd_stop("proxy-panel-hub").await?;
    }

    // Backup existing
    if hub_dst.exists() {
        let ver = installed_version(&hub_dst)
            .await?
            .unwrap_or_else(|| chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
        let bak = backup_path("proxy-panel-hub", &ver);
        ensure_dir(Path::new(BACKUP_DIR)).await?;
        copy_file(&hub_dst, &bak).await?;
    }
    if cli_dst.exists() {
        let ver = installed_version(&cli_dst)
            .await?
            .unwrap_or_else(|| chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
        let bak = backup_path("proxy-panel", &ver);
        ensure_dir(Path::new(BACKUP_DIR)).await?;
        copy_file(&cli_dst, &bak).await?;
    }

    // Move / copy new binaries
    if let Some(src) = hub_bin_src {
        move_file(&src, &hub_dst).await?;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&hub_dst, perms)
            .with_context(|| format!("failed to chmod {}", hub_dst.display()))?;
    } else {
        bail!("hub tarball 中未找到 proxy-panel-hub 二进制文件");
    }

    if let Some(src) = cli_bin_src {
        move_file(&src, &cli_dst).await?;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&cli_dst, perms)
            .with_context(|| format!("failed to chmod {}", cli_dst.display()))?;
    }

    // Copy web/dist
    if let Some(src) = web_dist_src {
        // Remove old dist and replace
        if web_dst.exists() {
            tokio::fs::remove_dir_all(&web_dst)
                .await
                .with_context(|| format!("failed to remove old {}", web_dst.display()))?;
        }
        move_path(&src, &web_dst).await?;
    }

    // Write hub.toml if not exists
    let hub_toml = PathBuf::from(ETC_DIR).join("hub.toml");
    if !hub_toml.exists() {
        let jwt_secret = pp_common::generate_secure_token();
        let toml_content = format!(
            r#"listen = "0.0.0.0:8081"
grpc_listen = "0.0.0.0:50052"
database_url = "postgres://proxypanel:CHANGE_ME@localhost/proxypanel"
static_dir = "/opt/proxy-panel/web/dist"
jwt_secret = "{}"
"#,
            jwt_secret
        );
        write_file(&hub_toml, &toml_content, None).await?;
    }

    // Write hub.env
    let hub_env = PathBuf::from(ETC_DIR).join("hub.env");
    let env_content = "RUST_LOG=proxy_panel_hub=info,tower_http=debug\n";
    write_file(&hub_env, env_content, Some(0o600)).await?;
    let _ = Command::new("chown")
        .arg(format!("{}:{}", USER_NAME, USER_NAME))
        .arg(hub_env.to_string_lossy().as_ref())
        .status()
        .await;

    // Write systemd unit
    let unit_path = PathBuf::from("/etc/systemd/system/proxy-panel-hub.service");
    write_file(&unit_path, HUB_UNIT, Some(0o644)).await?;
    systemd_daemon_reload().await?;

    // Fix ownership of /opt/proxy-panel
    let _ = Command::new("chown")
        .arg("-R")
        .arg(format!("{}:{}", USER_NAME, USER_NAME))
        .arg(OPT_DIR)
        .status()
        .await;

    // Restart only if the hub was running before (fresh installs wait for
    // the user to configure database_url first).
    if hub_was_active {
        systemd_restart("proxy-panel-hub").await?;
    }

    prune_backups("proxy-panel-hub").await;
    prune_backups("proxy-panel").await;

    println!("Hub 安装完成。");
    println!();
    println!("下一步：");
    println!("  1. 编辑 {} 填入正确的 database_url", hub_toml.display());
    println!("  2. 运行 proxy-panel init-db --database-url <url> 初始化数据库");
    println!("  3. 运行 systemctl enable --now proxy-panel-hub 启动服务");
    Ok(())
}

/// Install the Agent component.
pub async fn install_agent(
    hub_url: &str,
    token: &str,
    agent_id: Option<&str>,
    name: Option<&str>,
    version: &str,
    repo: &str,
) -> Result<()> {
    require_root()?;
    let arch = release_arch()?;
    let asset = format!("proxy-panel-agent-linux-{}.tar.gz", arch);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .context("failed to build HTTP client")?;

    let tmp_dir = tempfile::tempdir().context("failed to create temp dir")?;
    let archive = download_and_verify(&client, repo, version, &asset, tmp_dir.path()).await?;

    ensure_dir(Path::new(BIN_DIR)).await?;
    ensure_dir(Path::new(ETC_DIR)).await?;
    ensure_dir(Path::new(VAR_DIR)).await?;
    ensure_dir(Path::new(&format!("{}/agent", VAR_DIR))).await?;
    ensure_dir(Path::new(OPT_DIR)).await?;
    ensure_dir(Path::new(&format!("{}/bin", OPT_DIR))).await?;
    ensure_dir(Path::new(BACKUP_DIR)).await?;
    ensure_user().await?;

    let extract_dir = tmp_dir.path().join("extract");
    ensure_dir(&extract_dir).await?;
    extract_tar_gz(&archive, &extract_dir).await?;

    let mut agent_bin_src: Option<PathBuf> = None;
    let mut entries = tokio::fs::read_dir(&extract_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        if name == "proxy-panel-agent" {
            agent_bin_src = Some(entry.path());
        }
    }

    let agent_dst = PathBuf::from(BIN_DIR).join("proxy-panel-agent");

    // Stop the running service before replacing its binary (ETXTBSY).
    if systemd_is_active("proxy-panel-agent").await {
        systemd_stop("proxy-panel-agent").await?;
    }

    // Backup existing
    if agent_dst.exists() {
        let ver = installed_version(&agent_dst)
            .await?
            .unwrap_or_else(|| chrono::Local::now().format("%Y%m%d%H%M%S").to_string());
        let bak = backup_path("proxy-panel-agent", &ver);
        ensure_dir(Path::new(BACKUP_DIR)).await?;
        copy_file(&agent_dst, &bak).await?;
    }

    if let Some(src) = agent_bin_src {
        move_file(&src, &agent_dst).await?;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&agent_dst, perms)
            .with_context(|| format!("failed to chmod {}", agent_dst.display()))?;
    } else {
        bail!("agent tarball 中未找到 proxy-panel-agent 二进制文件");
    }

    // Write agent.env
    let agent_env = PathBuf::from(ETC_DIR).join("agent.env");
    let mut env_lines = vec![
        format!("PROXYPANEL_HUB_URL={}", hub_url),
        format!("PROXYPANEL_AGENT_TOKEN={}", token),
    ];
    if let Some(id) = agent_id {
        env_lines.push(format!("PROXYPANEL_AGENT_ID={}", id));
    }
    if let Some(n) = name {
        env_lines.push(format!("PROXYPANEL_AGENT_NAME={}", n));
    }
    env_lines.push("RUST_LOG=proxy_panel_agent=info".to_string());
    let env_content = env_lines.join("\n") + "\n";
    write_file(&agent_env, &env_content, Some(0o600)).await?;
    let _ = Command::new("chown")
        .arg(format!("{}:{}", USER_NAME, USER_NAME))
        .arg(agent_env.to_string_lossy().as_ref())
        .status()
        .await;

    // Write systemd unit
    let unit_path = PathBuf::from("/etc/systemd/system/proxy-panel-agent.service");
    write_file(&unit_path, AGENT_UNIT, Some(0o644)).await?;
    systemd_daemon_reload().await?;

    // Ownership
    let _ = Command::new("chown")
        .arg("-R")
        .arg(format!("{}:{}", USER_NAME, USER_NAME))
        .arg(VAR_DIR)
        .status()
        .await;

    // Fix ownership of /opt/proxy-panel (agent writes binaries here)
    let _ = Command::new("chown")
        .arg("-R")
        .arg(format!("{}:{}", USER_NAME, USER_NAME))
        .arg(OPT_DIR)
        .status()
        .await;

    systemd_enable_now("proxy-panel-agent").await?;

    if !systemd_is_active("proxy-panel-agent").await {
        let out = Command::new("journalctl")
            .args(["-u", "proxy-panel-agent", "-n", "20", "--no-pager"])
            .output()
            .await?;
        eprintln!(
            "Agent 启动失败，最近 20 条日志：\n{}",
            String::from_utf8_lossy(&out.stdout)
        );
        bail!("proxy-panel-agent 启动失败");
    }

    prune_backups("proxy-panel-agent").await;

    println!("Agent 安装并启动成功。");
    Ok(())
}
