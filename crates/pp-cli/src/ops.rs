//! Operations for installing, upgrading, uninstalling, and managing ProxyPanel components.

use anyhow::{Context, Result, bail};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BIN_DIR: &str = "/usr/local/bin";
const ETC_DIR: &str = "/etc/proxy-panel";
const OPT_DIR: &str = "/opt/proxy-panel";
const VAR_DIR: &str = "/var/lib/proxy-panel";
const BACKUP_DIR: &str = "/opt/proxy-panel/backup";
const USER_NAME: &str = "proxypanel";

const AGENT_UNIT: &str = include_str!("../../../deploy/proxy-panel-agent.service");
const HUB_UNIT: &str = include_str!("../../../deploy/proxy-panel-hub.service");

// ---------------------------------------------------------------------------
// Architecture helpers
// ---------------------------------------------------------------------------

/// Returns the release asset architecture suffix used in tarball names.
fn release_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x86_64"),
        "aarch64" => Ok("aarch64"),
        other => bail!("unsupported architecture: {}", other),
    }
}

// ---------------------------------------------------------------------------
// Version / URL helpers
// ---------------------------------------------------------------------------

/// Build the download URL for a release asset.
fn build_download_url(repo: &str, version: &str, asset: &str) -> String {
    if version == "latest" {
        format!(
            "https://github.com/{}/releases/latest/download/{}",
            repo, asset
        )
    } else {
        let tag = if version.starts_with('v') {
            version.to_string()
        } else {
            format!("v{}", version)
        };
        format!(
            "https://github.com/{}/releases/download/{}/{}",
            repo, tag, asset
        )
    }
}

/// Parse the expected SHA-256 hash for a given asset from the SHA256SUMS content.
fn parse_sha256_from_sums(sums_text: &str, asset_name: &str) -> Option<String> {
    for line in sums_text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Format: <hash>  <filename>  (two spaces typical, but allow any whitespace)
        let mut parts = line.split_whitespace();
        let (Some(hash), Some(filename)) = (parts.next(), parts.next()) else {
            continue;
        };
        if filename == asset_name {
            return Some(hash.to_lowercase());
        }
    }
    None
}

/// Compute SHA-256 of a file in a streaming fashion.
async fn sha256_file(path: &Path) -> Result<String> {
    let mut file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 8192];
    loop {
        let n = file
            .read(&mut buf)
            .await
            .with_context(|| format!("failed to read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Parse version string from `--version` stdout like "proxy-panel 0.3.3".
fn parse_version_from_output(output: &str) -> Option<String> {
    // Take the first line, then the last whitespace-separated token.
    let first = output.lines().next()?.trim();
    first.split_whitespace().last().map(|s| s.to_string())
}

/// Generate a backup file path for a binary.
fn backup_path(binary_name: &str, version_or_timestamp: &str) -> PathBuf {
    PathBuf::from(BACKUP_DIR).join(format!("{}.{}.bak", binary_name, version_or_timestamp))
}

/// Find the most recent backup for a component binary.
async fn find_latest_backup(binary_name: &str) -> Result<Option<PathBuf>> {
    let mut entries = tokio::fs::read_dir(BACKUP_DIR).await?;
    let mut candidates: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with(&format!("{}.", binary_name)) && name_str.ends_with(".bak") {
            let meta = entry.metadata().await?;
            if let Ok(modified) = meta.modified() {
                candidates.push((entry.path(), modified));
            }
        }
    }
    candidates.sort_by_key(|b| std::cmp::Reverse(b.1)); // newest first
    Ok(candidates.into_iter().next().map(|(p, _)| p))
}

// ---------------------------------------------------------------------------
// Root check
// ---------------------------------------------------------------------------

fn require_root() -> Result<()> {
    let uid = unsafe { libc::getuid() };
    if uid != 0 {
        bail!("此操作需要 root 权限，请使用 sudo 运行");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HTTP download helpers
// ---------------------------------------------------------------------------

async fn download_text(client: &reqwest::Client, url: &str) -> Result<String> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {}", url))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("下载失败 {}: HTTP {}", url, status);
    }
    resp.text()
        .await
        .with_context(|| format!("failed to read body from {}", url))
}

async fn download_to_file(client: &reqwest::Client, url: &str, dest: &Path) -> Result<()> {
    let resp = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("failed to GET {}", url))?;
    let status = resp.status();
    if !status.is_success() {
        bail!("下载失败 {}: HTTP {}", url, status);
    }
    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("failed to create {}", dest.display()))?;
    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("download stream error from {}", url))?;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("failed to write to {}", dest.display()))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Filesystem helpers
// ---------------------------------------------------------------------------

async fn ensure_dir(path: &Path) -> Result<()> {
    if !path.exists() {
        tokio::fs::create_dir_all(path)
            .await
            .with_context(|| format!("failed to create dir {}", path.display()))?;
    }
    Ok(())
}

async fn write_file(path: &Path, contents: &str, mode: Option<u32>) -> Result<()> {
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

async fn copy_file(src: &Path, dst: &Path) -> Result<()> {
    tokio::fs::copy(src, dst)
        .await
        .with_context(|| format!("failed to copy {} to {}", src.display(), dst.display()))
        .map(|_| ())
}

async fn remove_if_exists(path: &Path) -> Result<()> {
    if path.exists() {
        tokio::fs::remove_file(path)
            .await
            .with_context(|| format!("failed to remove {}", path.display()))?;
    }
    Ok(())
}

async fn remove_dir_if_empty(path: &Path) -> Result<()> {
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

// ---------------------------------------------------------------------------
// systemd helpers
// ---------------------------------------------------------------------------

async fn systemd_daemon_reload() -> Result<()> {
    let status = Command::new("systemctl")
        .arg("daemon-reload")
        .status()
        .await
        .context("failed to run systemctl daemon-reload")?;
    if !status.success() {
        bail!("systemctl daemon-reload failed");
    }
    Ok(())
}

async fn systemd_enable_now(unit: &str) -> Result<()> {
    let status = Command::new("systemctl")
        .args(["enable", "--now", unit])
        .status()
        .await
        .with_context(|| format!("failed to run systemctl enable --now {}", unit))?;
    if !status.success() {
        bail!("systemctl enable --now {} 失败", unit);
    }
    Ok(())
}

async fn systemd_stop(unit: &str) -> Result<()> {
    let _ = Command::new("systemctl")
        .args(["stop", unit])
        .status()
        .await;
    Ok(())
}

async fn systemd_disable(unit: &str) -> Result<()> {
    let _ = Command::new("systemctl")
        .args(["disable", unit])
        .status()
        .await;
    Ok(())
}

async fn systemd_restart(unit: &str) -> Result<()> {
    let status = Command::new("systemctl")
        .args(["restart", unit])
        .status()
        .await
        .with_context(|| format!("failed to run systemctl restart {}", unit))?;
    if !status.success() {
        bail!("systemctl restart {} 失败", unit);
    }
    Ok(())
}

async fn systemd_is_active(unit: &str) -> bool {
    match Command::new("systemctl")
        .args(["is-active", unit])
        .output()
        .await
    {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

async fn systemd_is_enabled(unit: &str) -> bool {
    match Command::new("systemctl")
        .args(["is-enabled", unit])
        .output()
        .await
    {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// User / group helper
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Tar extraction helper (spawns system tar)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Download + verify + install helpers
// ---------------------------------------------------------------------------

/// Download an asset and its SHA256SUMS, verify hash, return the local file path.
async fn download_and_verify(
    client: &reqwest::Client,
    repo: &str,
    version: &str,
    asset: &str,
    tmp_dir: &Path,
) -> Result<PathBuf> {
    let sums_url = build_download_url(repo, version, "SHA256SUMS");
    let sums_text = download_text(client, &sums_url).await?;
    let expected = parse_sha256_from_sums(&sums_text, asset)
        .with_context(|| format!("在 SHA256SUMS 中未找到 {}", asset))?;

    let asset_url = build_download_url(repo, version, asset);
    let asset_path = tmp_dir.join(asset);
    download_to_file(client, &asset_url, &asset_path).await?;

    let actual = sha256_file(&asset_path).await?;
    if actual != expected {
        bail!(
            "SHA-256 校验失败: {}\n期望: {}\n实际: {}",
            asset,
            expected,
            actual
        );
    }

    Ok(asset_path)
}

/// Get the installed version of a binary by running `<bin> --version`.
async fn installed_version(bin_path: &Path) -> Result<Option<String>> {
    if !bin_path.exists() {
        return Ok(None);
    }
    let out = Command::new(bin_path)
        .arg("--version")
        .output()
        .await
        .with_context(|| format!("failed to run {} --version", bin_path.display()))?;
    if !out.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(parse_version_from_output(&text))
}

/// Move a file, falling back to copy+atomic-rename across filesystems (EXDEV).
/// The fallback copies to a sibling temp file first, then renames over the
/// destination: rename(2) is atomic and is allowed to replace a running
/// binary (unlike O_TRUNC writes, which fail with ETXTBSY).
async fn move_file(src: &Path, dst: &Path) -> Result<()> {
    match tokio::fs::rename(src, dst).await {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            let file_name = dst
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "bin".to_string());
            let staging = dst.with_file_name(format!(".{}.new", file_name));
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
async fn move_path(src: &Path, dst: &Path) -> Result<()> {
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

async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
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

// ---------------------------------------------------------------------------
// Public commands
// ---------------------------------------------------------------------------

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

    println!("Agent 安装并启动成功。");
    Ok(())
}

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
                .arg(format!("{}:{}", USER_NAME, USER_NAME))
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

    let bak = find_latest_backup(bin_name)
        .await?
        .with_context(|| format!("未找到 {} 的备份文件", bin_name))?;

    let bin_path = PathBuf::from(BIN_DIR).join(bin_name);
    systemd_stop(unit).await?;
    move_file(&bak, &bin_path).await?;

    systemd_restart(unit).await?;
    if !systemd_is_active(unit).await {
        bail!("回滚后 {} 仍无法启动", unit);
    }

    println!("{} 已回滚并重启成功。", component);
    Ok(())
}

/// Uninstall a component.
pub async fn uninstall(component: &str, purge: bool) -> Result<()> {
    require_root()?;
    match component {
        "hub" => uninstall_hub(purge).await?,
        "agent" => uninstall_agent(purge).await?,
        other => bail!("未知组件: {}", other),
    }
    println!("{} 卸载完成。", component);
    Ok(())
}

async fn uninstall_hub(purge: bool) -> Result<()> {
    let unit = "proxy-panel-hub.service";
    systemd_stop(unit).await?;
    systemd_disable(unit).await?;
    remove_if_exists(Path::new("/etc/systemd/system").join(unit).as_path()).await?;
    systemd_daemon_reload().await?;

    remove_if_exists(Path::new(BIN_DIR).join("proxy-panel-hub").as_path()).await?;
    remove_if_exists(Path::new(ETC_DIR).join("hub.env").as_path()).await?;

    if purge {
        println!("注意：数据库数据不在本机删除范围内，请手动清理。");
    }

    cleanup_shared_dirs().await?;
    Ok(())
}

async fn uninstall_agent(purge: bool) -> Result<()> {
    let unit = "proxy-panel-agent.service";
    systemd_stop(unit).await?;
    systemd_disable(unit).await?;
    remove_if_exists(Path::new("/etc/systemd/system").join(unit).as_path()).await?;
    systemd_daemon_reload().await?;

    remove_if_exists(Path::new(BIN_DIR).join("proxy-panel-agent").as_path()).await?;
    remove_if_exists(Path::new(ETC_DIR).join("agent.env").as_path()).await?;

    if purge {
        let agent_data = Path::new(VAR_DIR).join("agent");
        if agent_data.exists() {
            tokio::fs::remove_dir_all(&agent_data)
                .await
                .with_context(|| format!("failed to remove {}", agent_data.display()))?;
        }
    }

    cleanup_shared_dirs().await?;
    Ok(())
}

async fn cleanup_shared_dirs() -> Result<()> {
    // Remove shared directories only if empty
    remove_dir_if_empty(Path::new(ETC_DIR)).await?;
    remove_dir_if_empty(Path::new(&format!("{}/bin", OPT_DIR))).await?;
    remove_dir_if_empty(Path::new(&format!("{}/web/dist", OPT_DIR))).await?;
    remove_dir_if_empty(Path::new(&format!("{}/web", OPT_DIR))).await?;
    remove_dir_if_empty(Path::new(OPT_DIR)).await?;
    remove_dir_if_empty(Path::new(VAR_DIR)).await?;
    Ok(())
}

/// Show status of installed components.
pub async fn status() -> Result<()> {
    println!(
        "{:<12} {:<10} {:<16} {:<10} {:<10} {:<12}",
        "组件", "已安装", "版本", "Active", "Enabled", "Env 文件"
    );
    println!("{}", "-".repeat(72));

    for (name, bin, unit, env) in [
        (
            "hub",
            "proxy-panel-hub",
            "proxy-panel-hub.service",
            "hub.env",
        ),
        (
            "agent",
            "proxy-panel-agent",
            "proxy-panel-agent.service",
            "agent.env",
        ),
        ("cli", "proxy-panel", "", ""),
    ] {
        let bin_path = PathBuf::from(BIN_DIR).join(bin);
        let installed = if bin_path.exists() { "是" } else { "否" };
        let version = if bin_path.exists() {
            installed_version(&bin_path)
                .await
                .unwrap_or(None)
                .unwrap_or_else(|| "未知".to_string())
        } else {
            "-".to_string()
        };
        let active = if unit.is_empty() {
            "-"
        } else if systemd_is_active(unit).await {
            "是"
        } else {
            "否"
        };
        let enabled = if unit.is_empty() {
            "-"
        } else if systemd_is_enabled(unit).await {
            "是"
        } else {
            "否"
        };
        let env_exists = if env.is_empty() {
            "-"
        } else if Path::new(ETC_DIR).join(env).exists() {
            "是"
        } else {
            "否"
        };

        println!(
            "{:<12} {:<10} {:<16} {:<10} {:<10} {:<12}",
            name, installed, version, active, enabled, env_exists
        );
    }

    Ok(())
}

/// Show logs for a component.
pub async fn logs(component: &str, lines: usize, follow: bool) -> Result<()> {
    let unit = match component {
        "hub" => "proxy-panel-hub.service",
        "agent" => "proxy-panel-agent.service",
        other => bail!("未知组件: {}", other),
    };

    let mut cmd = Command::new("journalctl");
    cmd.arg("-u").arg(unit);
    cmd.arg("-n").arg(lines.to_string());
    if follow {
        cmd.arg("-f");
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::inherit());
    cmd.stderr(Stdio::inherit());

    let status = cmd.status().await.context("failed to run journalctl")?;
    if !status.success() {
        bail!("journalctl 退出码非零");
    }
    Ok(())
}

/// Restart a component service.
pub async fn restart(component: &str) -> Result<()> {
    require_root()?;
    let unit = match component {
        "hub" => "proxy-panel-hub.service",
        "agent" => "proxy-panel-agent.service",
        other => bail!("未知组件: {}", other),
    };
    systemd_restart(unit).await?;
    println!("{} 已重启。", component);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_download_url_latest() {
        let url = build_download_url("owner/repo", "latest", "asset.tar.gz");
        assert_eq!(
            url,
            "https://github.com/owner/repo/releases/latest/download/asset.tar.gz"
        );
    }

    #[test]
    fn build_download_url_tagged() {
        let url = build_download_url("owner/repo", "v1.2.3", "asset.tar.gz");
        assert_eq!(
            url,
            "https://github.com/owner/repo/releases/download/v1.2.3/asset.tar.gz"
        );
    }

    #[test]
    fn build_download_url_tagged_no_v() {
        let url = build_download_url("owner/repo", "1.2.3", "asset.tar.gz");
        assert_eq!(
            url,
            "https://github.com/owner/repo/releases/download/v1.2.3/asset.tar.gz"
        );
    }

    #[test]
    fn parse_sha256_from_sums_ok() {
        let sums = "abc123  file.tar.gz\ndef456  other.tar.gz\n";
        assert_eq!(
            parse_sha256_from_sums(sums, "file.tar.gz"),
            Some("abc123".to_string())
        );
        assert_eq!(
            parse_sha256_from_sums(sums, "other.tar.gz"),
            Some("def456".to_string())
        );
        assert_eq!(parse_sha256_from_sums(sums, "missing.tar.gz"), None);
    }

    #[test]
    fn parse_sha256_skips_comments_and_empty() {
        let sums = "# comment\n\nabc123  file.tar.gz\n";
        assert_eq!(
            parse_sha256_from_sums(sums, "file.tar.gz"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn parse_version_from_output_ok() {
        assert_eq!(
            parse_version_from_output("proxy-panel 0.3.3"),
            Some("0.3.3".to_string())
        );
        assert_eq!(
            parse_version_from_output("proxy-panel-hub 0.3.3\n"),
            Some("0.3.3".to_string())
        );
        assert_eq!(
            parse_version_from_output("some-tool 1.0.0-beta.2"),
            Some("1.0.0-beta.2".to_string())
        );
    }

    #[test]
    fn parse_version_from_output_empty() {
        assert_eq!(parse_version_from_output(""), None);
        assert_eq!(parse_version_from_output("\n"), None);
    }

    #[test]
    fn backup_path_format() {
        let p = backup_path("proxy-panel-hub", "0.3.3");
        assert_eq!(
            p,
            PathBuf::from("/opt/proxy-panel/backup/proxy-panel-hub.0.3.3.bak")
        );
    }
}
