//! pp-script [`HttpExecutor`] 的 reqwest 实现。
//!
//! 供脚本宿主（[`ScriptHost`]）执行 `$task` / `$httpClient` 请求时使用；
//! 显式设置 `no_proxy()`，脚本拉取/执行请求不经系统代理（本客户端自身即代理）。

use std::time::Duration;

use async_trait::async_trait;
use pp_common::{PanelError, PanelResult};
use pp_script::{HttpExecutor, HttpRequestSpec, HttpResponseData};

/// 基于 reqwest 的 HTTP 执行器（脚本 `$task` / `$httpClient` 的真实网络实现）。
#[derive(Debug, Clone)]
pub struct ReqwestHttpExecutor {
    client: reqwest::Client,
}

impl Default for ReqwestHttpExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl ReqwestHttpExecutor {
    /// 创建执行器（30 秒请求超时，禁用系统代理）。
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client }
    }

    /// 使用自定义 HTTP 客户端创建执行器。
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }
}

#[async_trait]
impl HttpExecutor for ReqwestHttpExecutor {
    async fn execute(&self, req: HttpRequestSpec) -> PanelResult<HttpResponseData> {
        let method =
            reqwest::Method::from_bytes(req.method.as_bytes()).unwrap_or(reqwest::Method::GET);
        let mut builder = self.client.request(method, &req.url);
        for (name, value) in &req.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = &req.body {
            builder = builder.body(body.clone());
        }
        if let Some(timeout_ms) = req.timeout_ms {
            builder = builder.timeout(Duration::from_millis(timeout_ms));
        }
        let resp = builder.send().await.map_err(|e| {
            PanelError::Script(format!("http executor request failed ({}): {e}", req.url))
        })?;
        let status = resp.status().as_u16();
        let headers = resp
            .headers()
            .iter()
            .map(|(k, v)| {
                (
                    k.as_str().to_string(),
                    v.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();
        let body = resp.text().await.map_err(|e| {
            PanelError::Script(format!("http executor read body failed ({}): {e}", req.url))
        })?;
        Ok(HttpResponseData {
            status,
            headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn executes_get_and_returns_body() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/echo", axum::routing::get(|| async { "pong" }));
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let executor = ReqwestHttpExecutor::new();
        let resp = executor
            .execute(HttpRequestSpec {
                url: format!("http://{addr}/echo"),
                method: "GET".to_string(),
                headers: vec![],
                body: None,
                timeout_ms: Some(3000),
            })
            .await
            .unwrap();
        assert_eq!(resp.status, 200);
        assert_eq!(resp.body, "pong");
    }
}
