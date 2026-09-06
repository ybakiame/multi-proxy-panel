use pp_common::{PanelError, PanelResult};

use super::{FetchResult, parse_subscription_body};

/// Subscription fetcher.
#[derive(Debug, Clone)]
pub struct SubscriptionFetcher {
    client: reqwest::Client,
}

impl Default for SubscriptionFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionFetcher {
    /// Create a default subscription fetcher (15s request timeout, disable
    /// system proxy).
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// Create fetcher with custom HTTP client.
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Generic subscription fetch: any URL, auto sniff format (see
    /// [`fetch_subscription`](super::fetch_subscription)).
    ///
    /// GitHub blob/raw links are normalized to `raw.githubusercontent.com`
    /// before request.
    pub async fn fetch(&self, url: &str) -> PanelResult<FetchResult> {
        let url = crate::normalize_resource_url(url);
        tracing::debug!(url = %url, "fetching subscription");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| PanelError::Client(format!("subscription request failed: {e}")))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(PanelError::Client(format!(
                "subscription request returned HTTP {status}"
            )));
        }
        let info = super::parse_subscription_userinfo(resp.headers());
        let text = resp
            .text()
            .await
            .map_err(|e| PanelError::Client(format!("failed to read subscription body: {e}")))?;
        parse_subscription_body(&text, info)
    }
}
