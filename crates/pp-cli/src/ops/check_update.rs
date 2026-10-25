//! Non-privileged installed-version and stable-release checks.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pp_common::release::GitHubReleaseClient;
use serde::Serialize;

use super::BIN_DIR;
use super::download::installed_version_strict;

#[derive(Debug, Serialize)]
struct ComponentUpdate {
    component: String,
    current: Option<String>,
    latest: String,
    update_available: bool,
    release_url: String,
}

/// Check one or all components without requiring root.
pub async fn check_update(component: Option<&str>, repo: &str, json: bool) -> Result<()> {
    let components: Vec<&str> = match component {
        Some("hub") => vec!["hub"],
        Some("agent") => vec!["agent"],
        Some("cli") => vec!["cli"],
        Some(other) => bail!("未知组件: {}", other),
        None => vec!["hub", "agent", "cli"],
    };
    let client = GitHubReleaseClient::new(repo).context("failed to configure release client")?;
    let release = client.latest_stable().await?;
    let mut updates = Vec::new();
    for component in components {
        let binary_name = match component {
            "hub" => "proxy-panel-hub",
            "agent" => "proxy-panel-agent",
            "cli" => "proxy-panel",
            _ => unreachable!(),
        };
        let path = PathBuf::from(BIN_DIR).join(binary_name);
        let current = installed_or_development_version(component, &path).await?;
        let update_available = match current.as_deref() {
            Some(current) => {
                pp_common::release::compare_versions(current, &release.version.to_string())?.is_lt()
            }
            None => true,
        };
        updates.push(ComponentUpdate {
            component: component.to_string(),
            current,
            latest: release.version.to_string(),
            update_available,
            release_url: release.html_url.clone(),
        });
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&updates)?);
    } else {
        for update in updates {
            println!("{}:", update.component);
            println!(
                "  current: {}",
                update.current.as_deref().unwrap_or("not installed")
            );
            println!("  latest: {}", update.latest);
            println!("  update available: {}", update.update_available);
            println!("  release: {}", update.release_url);
        }
    }
    Ok(())
}

async fn installed_or_development_version(component: &str, path: &Path) -> Result<Option<String>> {
    if path.exists() {
        return installed_version_strict(path).await;
    }
    if component == "cli" {
        Ok(Some(env!("CARGO_PKG_VERSION").to_string()))
    } else {
        Ok(None)
    }
}
