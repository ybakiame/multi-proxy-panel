//! GitHub stable release queries and semantic-version helpers.

use std::cmp::Ordering;
use std::time::Duration;

use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::{PanelError, PanelResult};

const GITHUB_API: &str = "https://api.github.com";
const USER_AGENT: &str = concat!("proxy-panel/", env!("CARGO_PKG_VERSION"));

/// A stable GitHub release with its parsed semantic version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Release {
    pub tag: String,
    pub version: Version,
    pub html_url: String,
    pub published_at: Option<DateTime<Utc>>,
}

/// Client scoped to one validated GitHub `owner/name` repository.
pub struct GitHubReleaseClient {
    client: reqwest::Client,
    repo: String,
    api_base: String,
}

impl GitHubReleaseClient {
    pub fn new(repo: &str) -> PanelResult<Self> {
        validate_repo(repo)?;
        let api_base = std::env::var("PROXYPANEL_RELEASE_API_BASE")
            .unwrap_or_else(|_| GITHUB_API.to_string())
            .trim_end_matches('/')
            .to_string();
        if !(api_base.starts_with("https://") || api_base.starts_with("http://")) {
            return Err(PanelError::Validation(
                "PROXYPANEL_RELEASE_API_BASE must be an HTTP(S) URL".to_string(),
            ));
        }
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| {
                PanelError::Release(format!("failed to build GitHub HTTP client: {error}"))
            })?;
        Ok(Self {
            client,
            repo: repo.to_string(),
            api_base,
        })
    }

    /// Return the newest stable, non-draft release.
    pub async fn latest_stable(&self) -> PanelResult<Release> {
        let url = format!(
            "{}/repos/{}/releases?per_page=100",
            self.api_base, self.repo
        );
        let responses: Vec<ReleaseResponse> = self.get_json(&url).await?;
        select_latest_stable(responses)
    }

    /// Return a non-draft release by tag, accepting tags with or without `v`.
    /// Explicit prereleases are allowed for CLI testing; latest_stable remains stable-only.
    pub async fn by_tag(&self, tag: &str) -> PanelResult<Release> {
        parse_version(tag)?;
        let tag = if tag.starts_with('v') {
            tag.to_string()
        } else {
            format!("v{tag}")
        };
        let url = format!("{}/repos/{}/releases/tags/{tag}", self.api_base, self.repo);
        let response: ReleaseResponse = self.get_json(&url).await?;
        if response.draft {
            return Err(PanelError::Release(format!(
                "release {tag} is a draft and cannot be installed"
            )));
        }
        Release::try_from(response)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> PanelResult<T> {
        let response = self.client.get(url).send().await.map_err(|error| {
            PanelError::Release(format!("GitHub request failed for {url}: {error}"))
        })?;
        let status = response.status();
        if !status.is_success() {
            return Err(PanelError::Release(format!(
                "GitHub request failed for {url}: HTTP {status}"
            )));
        }
        response.json().await.map_err(|error| {
            PanelError::Release(format!("invalid GitHub JSON response from {url}: {error}"))
        })
    }
}

/// Parse a version, accepting the optional release-tag `v` prefix.
pub fn parse_version(value: &str) -> PanelResult<Version> {
    let normalized = value.strip_prefix('v').unwrap_or(value);
    Version::parse(normalized).map_err(|error| {
        PanelError::Validation(format!("invalid semantic version '{value}': {error}"))
    })
}

/// Compare two semantic versions, accepting optional `v` prefixes.
pub fn compare_versions(current: &str, target: &str) -> PanelResult<Ordering> {
    Ok(parse_version(current)?.cmp(&parse_version(target)?))
}

pub fn validate_repo(repo: &str) -> PanelResult<()> {
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && !matches!(segment, "." | "..")
            && segment
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
    };
    let mut segments = repo.split('/');
    let valid = matches!(segments.next(), Some(owner) if valid_segment(owner))
        && matches!(segments.next(), Some(name) if valid_segment(name))
        && segments.next().is_none();
    if valid {
        Ok(())
    } else {
        Err(PanelError::Validation(format!(
            "invalid GitHub repository '{repo}'; expected owner/name"
        )))
    }
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    html_url: String,
    published_at: Option<DateTime<Utc>>,
    draft: bool,
    prerelease: bool,
}

impl TryFrom<ReleaseResponse> for Release {
    type Error = PanelError;

    fn try_from(response: ReleaseResponse) -> PanelResult<Self> {
        let version = parse_version(&response.tag_name)?;
        Ok(Self {
            tag: response.tag_name,
            version,
            html_url: response.html_url,
            published_at: response.published_at,
        })
    }
}

fn select_latest_stable(responses: Vec<ReleaseResponse>) -> PanelResult<Release> {
    responses
        .into_iter()
        .filter(|response| !response.draft && !response.prerelease)
        .map(Release::try_from)
        .collect::<PanelResult<Vec<_>>>()?
        .into_iter()
        .max_by(|left, right| left.version.cmp(&right.version))
        .ok_or_else(|| PanelError::Release("no stable GitHub releases found".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(tag: &str, prerelease: bool) -> ReleaseResponse {
        ReleaseResponse {
            tag_name: tag.to_string(),
            html_url: format!("https://github.com/owner/repo/releases/tag/{tag}"),
            published_at: None,
            draft: false,
            prerelease,
        }
    }

    #[test]
    fn latest_stable_excludes_prereleases() {
        let release = select_latest_stable(vec![
            response("v1.2.0", false),
            response("v2.0.0-rc.1", true),
            response("v1.3.0", false),
        ])
        .unwrap();
        assert_eq!(release.version, Version::new(1, 3, 0));
    }

    #[test]
    fn explicit_prerelease_versions_are_valid() {
        let release = Release::try_from(response("v2.0.0-rc.1", true)).unwrap();
        assert_eq!(release.tag, "v2.0.0-rc.1");
        assert!(!release.version.pre.is_empty());
    }

    #[test]
    fn versions_accept_v_prefix_and_compare_semantically() {
        assert_eq!(parse_version("v1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(
            compare_versions("1.10.0", "v1.9.9").unwrap(),
            Ordering::Greater
        );
    }

    #[test]
    fn repository_names_are_validated() {
        assert!(validate_repo("owner/repo").is_ok());
        assert!(validate_repo("owner/repo/extra").is_err());
        assert!(validate_repo("../repo").is_err());
        assert!(validate_repo("owner/").is_err());
    }
}
