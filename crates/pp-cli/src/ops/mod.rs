//! Operations for installing, upgrading, uninstalling, and managing ProxyPanel components.

pub mod download;
pub mod fsutil;
pub mod install;
pub mod status_logs;
pub mod systemd;
pub mod tests;
pub mod uninstall;
pub mod upgrade;

pub use install::{install_agent, install_hub};
pub use status_logs::{logs, restart, status};
pub use uninstall::uninstall;
pub use upgrade::{rollback, upgrade};

use anyhow::{Result, bail};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const BIN_DIR: &str = "/usr/local/bin";
pub const ETC_DIR: &str = "/etc/proxy-panel";
pub const OPT_DIR: &str = "/opt/proxy-panel";
pub const VAR_DIR: &str = "/var/lib/proxy-panel";
pub const BACKUP_DIR: &str = "/opt/proxy-panel/backup";
pub const USER_NAME: &str = "proxypanel";

pub const AGENT_UNIT: &str = include_str!("../../../../deploy/proxy-panel-agent.service");
pub const HUB_UNIT: &str = include_str!("../../../../deploy/proxy-panel-hub.service");

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

/// Build the download URL for a release asset.
pub fn build_download_url(repo: &str, version: &str, asset: &str) -> String {
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

// ---------------------------------------------------------------------------
// Architecture helpers
// ---------------------------------------------------------------------------

/// Returns the release asset architecture suffix used in tarball names.
pub fn release_arch() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("x86_64"),
        "aarch64" => Ok("aarch64"),
        other => bail!("unsupported architecture: {}", other),
    }
}

// ---------------------------------------------------------------------------
// Root check
// ---------------------------------------------------------------------------

pub fn require_root() -> Result<()> {
    let uid = unsafe { libc::getuid() };
    if uid != 0 {
        bail!("此操作需要 root 权限，请使用 sudo 运行");
    }
    Ok(())
}
