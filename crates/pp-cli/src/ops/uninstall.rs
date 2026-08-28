//! Uninstall commands and cleanup helpers.

use anyhow::{Context, Result};
use std::path::Path;

use super::{BIN_DIR, ETC_DIR, OPT_DIR, VAR_DIR, require_root};
use super::fsutil::{remove_dir_if_empty, remove_if_exists};
use super::systemd::{systemd_daemon_reload, systemd_disable, systemd_stop};

/// Uninstall a component.
pub async fn uninstall(component: &str, purge: bool) -> Result<()> {
    require_root()?;
    match component {
        "hub" => uninstall_hub(purge).await?,
        "agent" => uninstall_agent(purge).await?,
        other => anyhow::bail!("未知组件: {}", other),
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
