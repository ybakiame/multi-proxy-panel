//! General subscription management: multi-subscription storage
//! (`data_dir/subscriptions.json`), format sniffing and URL fetching.
//!
//! Subscriptions are no longer bound to Hub: `fetch_subscription` accepts any
//! subscription URL, sniffs content for share links / clash YAML / sing-box JSON
//! three formats, and uniformly produces dual-core nodes.
//! The legacy Hub path (`/sub/{token}?format=...`) is retained as
//! [`SubscriptionFetcher`] compatibility methods, used by `state` as fallback
//! when no generic subscription is configured.

use std::time::Duration;

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::node_convert::{mihomo_to_singbox, singbox_to_mihomo};
use crate::profile::{extract_nodes_mihomo, extract_nodes_singbox};
use crate::share_link::{ShareLinkParseResult, parse_share_links};

mod fetch;
mod store;
#[cfg(test)]
mod tests;

pub use fetch::*;
pub use store::*;

/// Subscription user info, from the `subscription-userinfo` response header.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    /// Uploaded bytes.
    pub upload: Option<u64>,
    /// Downloaded bytes.
    pub download: Option<u64>,
    /// Total traffic bytes.
    pub total: Option<u64>,
    /// Expiration timestamp (seconds).
    pub expire: Option<u64>,
}

/// A subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub id: Uuid,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub userinfo: Option<SubscriptionInfo>,
    /// Number of nodes from the last successful fetch (sing-box side available
    /// nodes); `0` = not yet fetched successfully.
    ///
    /// `#[serde(default)]` ensures legacy `subscriptions.json` (without this
    /// field) deserializes normally.
    #[serde(default)]
    pub node_count: u64,
    /// Error message from the last fetch (recorded on failure, does not block
    /// existing data display).
    ///
    /// `#[serde(default)]` ensures legacy `subscriptions.json` (without this
    /// field) deserializes normally.
    #[serde(default)]
    pub error: Option<String>,
    /// User-Agent used when fetching; `None` / empty string uses default
    /// `clash.meta`.
    ///
    /// Some subscription sources return different formats based on UA, which
    /// can be customized (`#[serde(default)]` ensures legacy
    /// `subscriptions.json` (without this field) deserializes normally).
    #[serde(default)]
    pub user_agent: Option<String>,
    /// Sniffed subscription content format from the last fetch (`ShareLinks` /
    /// `ClashYaml` / `SingBoxJson`); `None` when not yet fetched successfully.
    ///
    /// `#[serde(default)]` ensures legacy `subscriptions.json` (without this
    /// field) deserializes normally.
    #[serde(default)]
    pub format: Option<SubFormat>,
    /// Associated override template id (`data_dir/profiles.json`); `None` = no
    /// override.
    ///
    /// Pure association: the override used at runtime = the override template
    /// associated with the currently selected subscription.
    /// `#[serde(default)]` ensures legacy `subscriptions.json` (without this
    /// field) deserializes normally.
    #[serde(default)]
    pub profile_id: Option<Uuid>,
}

/// Subscription content format (sniffing result).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubFormat {
    /// Share links (base64 or plaintext line list).
    ShareLinks,
    /// clash/mihomo YAML (contains `proxies:`).
    ClashYaml,
    /// sing-box JSON (contains `outbounds`).
    SingBoxJson,
}

impl Default for SubFormat {
    /// Default format is share links (both cores support it, used as fallback
    /// when old cache lacks `format` field).
    fn default() -> Self {
        Self::ShareLinks
    }
}

/// Subscription fetch result: sniffed format + dual-core nodes + user info +
/// line-level warnings.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub format: SubFormat,
    pub singbox_nodes: Vec<Value>,
    pub mihomo_nodes: Vec<Value>,
    pub userinfo: Option<SubscriptionInfo>,
    pub warnings: Vec<String>,
}

/// Subscription content cache: written to disk
/// (`data_dir/subscription_cache/<id>.json`) on successful refresh with
/// dual-core nodes + sniffed format.
///
/// Used for config preview etc. to assemble locally and avoid remote fetching
/// every time. All fields have `#[serde(default)]` to ensure compatibility
/// with old cache files missing fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedSubscriptionContent {
    /// Sniffed subscription format (same as [`Subscription::format`]).
    #[serde(default)]
    pub format: SubFormat,
    /// sing-box side available nodes (`outbounds` array elements).
    #[serde(default)]
    pub singbox_nodes: Vec<Value>,
    /// mihomo side nodes (`proxies` array elements).
    #[serde(default)]
    pub mihomo_nodes: Vec<Value>,
}

/// Generic subscription fetch (module-level entry): GET (no_proxy, 30s timeout)
/// → sniff format.
///
/// `ua` is `None` / empty string uses default UA `clash.meta`; some
/// subscription sources return different formats based on UA, which can be
/// customized.
pub async fn fetch_subscription_with_ua(url: &str, ua: Option<&str>) -> PanelResult<FetchResult> {
    let ua = ua
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("clash.meta");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .user_agent(ua)
        .build()
        .map_err(|e| PanelError::Client(format!("failed to build http client: {e}")))?;
    SubscriptionFetcher::with_client(client).fetch(url).await
}

/// Generic subscription fetch (module-level entry): GET (no_proxy, 30s timeout,
/// default UA "clash.meta") → sniff format.
pub async fn fetch_subscription(url: &str) -> PanelResult<FetchResult> {
    fetch_subscription_with_ua(url, None).await
}

/// Parse `subscription-userinfo` response header
/// (`upload=..; download=..; total=..; expire=..`).
pub fn parse_subscription_userinfo(
    headers: &reqwest::header::HeaderMap,
) -> Option<SubscriptionInfo> {
    let value = headers.get("subscription-userinfo")?.to_str().ok()?;
    let mut info = SubscriptionInfo::default();
    for pair in value.split(';') {
        let Some((key, val)) = pair.split_once('=') else {
            continue;
        };
        match (key.trim(), val.trim()) {
            ("upload", v) => info.upload = v.parse().ok(),
            ("download", v) => info.download = v.parse().ok(),
            ("total", v) => info.total = v.parse().ok(),
            ("expire", v) => info.expire = v.parse().ok(),
            _ => {}
        }
    }
    Some(info)
}

/// Sniff subscription content format and convert to dual-core nodes:
///
/// ① Overall base64 decode succeeds and contains `://` → share links
/// ② Contains `proxies:` → clash YAML (proxies → dual format)
/// ③ JSON contains `outbounds` → sing-box JSON (extract then convert to mihomo)
/// ④ Lines contain `://` → plaintext share links
pub fn parse_subscription_body(
    text: &str,
    info: Option<SubscriptionInfo>,
) -> PanelResult<FetchResult> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(PanelError::Client("subscription body is empty".to_string()));
    }

    // ① Base64 share link list.
    if let Some(decoded) = crate::share_link::b64_decode(trimmed)
        && decoded.contains("://")
    {
        return Ok(share_links_result(
            parse_share_links(&decoded),
            info,
            SubFormat::ShareLinks,
        ));
    }

    // ② clash YAML.
    if trimmed.contains("proxies:") {
        let proxies = extract_nodes_mihomo(trimmed)?;
        let mut singbox_nodes = Vec::with_capacity(proxies.len());
        let mut warnings = Vec::new();
        for p in &proxies {
            match mihomo_to_singbox(p) {
                Some(o) => singbox_nodes.push(o),
                None => {
                    if let Some(n) = p.get("name").and_then(Value::as_str) {
                        warnings.push(format!("unsupported clash proxy type skipped: {n}"));
                    }
                }
            }
        }
        return Ok(FetchResult {
            format: SubFormat::ClashYaml,
            singbox_nodes,
            mihomo_nodes: proxies,
            userinfo: info,
            warnings,
        });
    }

    // ③ sing-box JSON.
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && let Ok(parsed) = serde_json::from_str::<Value>(trimmed)
        && parsed.get("outbounds").and_then(Value::as_array).is_some()
    {
        let outbounds = extract_nodes_singbox(&parsed);
        let mut mihomo_nodes = Vec::with_capacity(outbounds.len());
        let mut warnings = Vec::new();
        for o in &outbounds {
            match singbox_to_mihomo(o) {
                Some(p) => mihomo_nodes.push(p),
                None => {
                    if let Some(t) = o.get("tag").and_then(Value::as_str) {
                        warnings.push(format!("unsupported sing-box outbound type skipped: {t}"));
                    }
                }
            }
        }
        return Ok(FetchResult {
            format: SubFormat::SingBoxJson,
            singbox_nodes: outbounds,
            mihomo_nodes,
            userinfo: info,
            warnings,
        });
    }

    // ④ Plaintext share links.
    if trimmed.lines().any(|l| l.contains("://")) {
        return Ok(share_links_result(
            parse_share_links(trimmed),
            info,
            SubFormat::ShareLinks,
        ));
    }

    Err(PanelError::Client(
        "unrecognized subscription format".to_string(),
    ))
}

/// Share link parse result → [`FetchResult`].
fn share_links_result(
    parsed: ShareLinkParseResult,
    info: Option<SubscriptionInfo>,
    format: SubFormat,
) -> FetchResult {
    let mut singbox_nodes = Vec::with_capacity(parsed.nodes.len());
    let mut mihomo_nodes = Vec::with_capacity(parsed.nodes.len());
    for node in &parsed.nodes {
        singbox_nodes.push(node.outbound_singbox.clone());
        mihomo_nodes.push(node.proxy_mihomo.clone());
    }
    FetchResult {
        format,
        singbox_nodes,
        mihomo_nodes,
        userinfo: info,
        warnings: parsed.warnings,
    }
}
