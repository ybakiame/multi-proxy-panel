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

/// Wait for a restarted unit to remain active for three consecutive checks.
pub async fn systemd_wait_stable(unit: &str) -> Result<()> {
    let mut stable_checks = 0;
    for _ in 0..30 {
        if systemd_is_active(unit).await {
            stable_checks += 1;
            if stable_checks == 3 {
                return Ok(());
            }
        } else {
            stable_checks = 0;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    bail!("{} 未能在 30 秒内稳定保持 active", unit)
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
