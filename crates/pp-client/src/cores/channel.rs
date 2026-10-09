//! Release channel model (稳定版 / 测试版 / 预发布版) for core management.
//!
//! Channel design references `.reference/GUI.for.SingBox` (Stable / Alpha branches):
//! remote releases are classified by tag suffix and each channel tracks its latest
//! release, so users never pick a concrete version number manually.

use pp_common::{CoreType, PanelError, PanelResult};
use serde::{Deserialize, Serialize};

use super::ClientCoreInventory;

/// Release channel of a sing-box version (aligned with the desktop core-management
/// channel model: 稳定版 / 测试版 / 预发布版, see `.reference/GUI.for.SingBox`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoreChannel {
    /// 稳定版：正式 release（无预发布后缀）。
    Stable,
    /// 测试版：`-beta.*` / `-rc.*` 预发布。
    Beta,
    /// 预发布版：`-alpha.*`（或其它未识别预发布后缀）。
    Prerelease,
}

impl CoreChannel {
    /// Frontend string representation: `stable` / `beta` / `prerelease`.
    pub fn as_str(&self) -> &'static str {
        match self {
            CoreChannel::Stable => "stable",
            CoreChannel::Beta => "beta",
            CoreChannel::Prerelease => "prerelease",
        }
    }
}

/// Classify a version tag into a release channel by its prerelease marker:
/// no `-` suffix → [`CoreChannel::Stable`]; marker starting with `beta` / `rc` →
/// [`CoreChannel::Beta`]; anything else (`alpha`, …) → [`CoreChannel::Prerelease`].
///
/// Pure tag-based classification (the GitHub `prerelease` flag is not consulted) so the
/// frontend can classify locally installed versions with the identical rule.
pub fn channel_of_version(version: &str) -> CoreChannel {
    let v = version.trim().trim_start_matches('v').to_ascii_lowercase();
    let Some(idx) = v.find('-') else {
        return CoreChannel::Stable;
    };
    let marker = &v[idx + 1..];
    if marker.starts_with("beta") || marker.starts_with("rc") {
        CoreChannel::Beta
    } else {
        CoreChannel::Prerelease
    }
}

/// Latest remote version of one release channel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteChannelVersion {
    pub channel: CoreChannel,
    /// Version with `v` prefix stripped.
    pub version: String,
}

impl ClientCoreInventory {
    /// List the latest remote version of each release channel (stable / beta /
    /// prerelease), referencing the GUI.for.SingBox branch model.
    ///
    /// Fetches the recent 30 releases (GitHub returns newest first by creation time);
    /// the first match per channel wins. Channels without any matching release in the
    /// window are omitted from the result.
    pub async fn list_remote_channels(&self) -> PanelResult<Vec<RemoteChannelVersion>> {
        let (owner, repo) = CoreType::SingBox.github_repo();
        let url = format!(
            "{}/repos/{}/{}/releases?per_page=30",
            self.api_base, owner, repo
        );
        // GitHub API URL is wrapped by configured proxy prefix (shares GitHub access strategy
        // with remote resource fetching); injected mock service addresses (non-GitHub domains)
        // are not affected.
        let url = crate::apply_github_proxy_prefix(&url, &self.github_proxy_prefix());
        let resp = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(super::HTTP_TIMEOUT_SECS))
            .header("User-Agent", "proxy-panel-client")
            .send()
            .await
            .map_err(|e| PanelError::Core(format!("GitHub API request failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(PanelError::Core(format!(
                "GitHub API returned status {}",
                resp.status()
            )));
        }
        let releases: Vec<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| PanelError::Core(format!("Failed to parse GitHub releases: {e}")))?;
        let mut channels: Vec<RemoteChannelVersion> = Vec::new();
        for release in releases {
            let Some(tag) = release.get("tag_name").and_then(|v| v.as_str()) else {
                continue;
            };
            let version = tag.strip_prefix('v').unwrap_or(tag);
            if version.is_empty() {
                continue;
            }
            let channel = channel_of_version(version);
            if channels
                .iter()
                .any(|c: &RemoteChannelVersion| c.channel == channel)
            {
                continue;
            }
            channels.push(RemoteChannelVersion {
                channel,
                version: version.to_string(),
            });
        }
        Ok(channels)
    }
}
