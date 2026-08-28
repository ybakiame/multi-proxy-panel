//! systemd helper functions.

use anyhow::{Context, Result, bail};
use tokio::process::Command;

/// Reload systemd daemon configuration.
pub async fn systemd_daemon_reload() -> Result<()> {
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

/// Enable and start a systemd unit immediately.
pub async fn systemd_enable_now(unit: &str) -> Result<()> {
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

/// Stop a systemd unit (ignores errors).
pub async fn systemd_stop(unit: &str) -> Result<()> {
    let _ = Command::new("systemctl")
        .args(["stop", unit])
        .status()
        .await;
    Ok(())
}

/// Disable a systemd unit (ignores errors).
pub async fn systemd_disable(unit: &str) -> Result<()> {
    let _ = Command::new("systemctl")
        .args(["disable", unit])
        .status()
        .await;
    Ok(())
}

/// Restart a systemd unit.
pub async fn systemd_restart(unit: &str) -> Result<()> {
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

/// Check whether a systemd unit is currently active.
pub async fn systemd_is_active(unit: &str) -> bool {
    match Command::new("systemctl")
        .args(["is-active", unit])
        .output()
        .await
    {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}

/// Check whether a systemd unit is enabled.
pub async fn systemd_is_enabled(unit: &str) -> bool {
    match Command::new("systemctl")
        .args(["is-enabled", unit])
        .output()
        .await
    {
        Ok(out) => out.status.success(),
        Err(_) => false,
    }
}
