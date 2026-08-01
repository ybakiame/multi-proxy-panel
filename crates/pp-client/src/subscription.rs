//! 订阅拉取与响应解析。

use std::time::Duration;

use pp_common::{PanelError, PanelResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 订阅用户信息，来自 `subscription-userinfo` 响应头。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionInfo {
    /// 已用上行字节数。
    pub upload: Option<u64>,
    /// 已用下行字节数。
    pub download: Option<u64>,
    /// 总流量字节数。
    pub total: Option<u64>,
    /// 到期时间戳（秒）。
    pub expire: Option<u64>,
}

/// 订阅拉取器。
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
    /// 创建一个默认的订阅拉取器（15 秒请求超时）。
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// 使用自定义 HTTP 客户端创建拉取器。
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// 拉取 sing-box 订阅配置，返回配置 JSON 与可选的用户信息。
    pub async fn fetch_singbox_config(
        &self,
        hub_url: &str,
        token: &str,
    ) -> PanelResult<(Value, Option<SubscriptionInfo>)> {
        let url = format!(
            "{}/sub/{}?format=singbox",
            hub_url.trim_end_matches('/'),
            token
        );
        tracing::debug!(url = %url, "fetching subscription sing-box config");

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

        let info = parse_subscription_userinfo(resp.headers());
        let text = resp
            .text()
            .await
            .map_err(|e| PanelError::Client(format!("failed to read subscription body: {e}")))?;
        let config: Value = serde_json::from_str(&text).map_err(|e| {
            PanelError::Client(format!("invalid sing-box config in subscription: {e}"))
        })?;

        Ok((config, info))
    }
}

/// 解析 `subscription-userinfo` 响应头（`upload=..; download=..; total=..; expire=..`）。
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

#[cfg(test)]
mod tests {
    use super::*;

    const SUB_JSON: &str = r#"{
        "log": { "level": "info" },
        "outbounds": [
            { "type": "vless", "tag": "n1", "server": "example.com", "server_port": 443 }
        ],
        "route": { "final": "n1" }
    }"#;

    async fn spawn_server(app: axum::Router) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn fetch_singbox_config_parses_config_and_info() {
        let app = axum::Router::new().route(
            "/sub/{token}",
            axum::routing::get(|| async {
                (
                    [(
                        "subscription-userinfo",
                        "upload=100; download=200; total=1000; expire=1700000000",
                    )],
                    SUB_JSON,
                )
            }),
        );
        let base = spawn_server(app).await;

        let fetcher = SubscriptionFetcher::new();
        let (config, info) = fetcher.fetch_singbox_config(&base, "tok").await.unwrap();

        assert_eq!(config["outbounds"][0]["tag"], "n1");
        assert_eq!(config["route"]["final"], "n1");

        let info = info.unwrap();
        assert_eq!(info.upload, Some(100));
        assert_eq!(info.download, Some(200));
        assert_eq!(info.total, Some(1000));
        assert_eq!(info.expire, Some(1700000000));
    }

    #[tokio::test]
    async fn fetch_singbox_config_returns_client_error_on_4xx() {
        let app = axum::Router::new().route(
            "/sub/{token}",
            axum::routing::get(|| async { axum::http::StatusCode::NOT_FOUND }),
        );
        let base = spawn_server(app).await;

        let fetcher = SubscriptionFetcher::new();
        let err = fetcher
            .fetch_singbox_config(&base, "missing")
            .await
            .unwrap_err();
        assert!(matches!(err, PanelError::Client(_)));
    }

    #[test]
    fn parse_userinfo_ignores_malformed_pairs() {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static("subscription-userinfo"),
            reqwest::header::HeaderValue::from_static("upload=1; bad; total=1024; expire=5"),
        );
        let info = parse_subscription_userinfo(&headers).unwrap();
        assert_eq!(info.upload, Some(1));
        assert_eq!(info.total, Some(1024));
        assert_eq!(info.expire, Some(5));
        assert_eq!(info.download, None);
    }
}
