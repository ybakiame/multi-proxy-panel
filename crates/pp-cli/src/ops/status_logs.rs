//! Status, logs, and restart commands.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

use super::download::installed_version;
use super::systemd::{systemd_is_active, systemd_is_enabled, systemd_restart};
use super::{BIN_DIR, ETC_DIR};

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
        let bin_path = Path::new(BIN_DIR).join(bin);
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
    super::require_root()?;
    let unit = match component {
        "hub" => "proxy-panel-hub.service",
        "agent" => "proxy-panel-agent.service",
        other => bail!("未知组件: {}", other),
    };
    systemd_restart(unit).await?;
    println!("{} 已重启。", component);
    Ok(())
}
