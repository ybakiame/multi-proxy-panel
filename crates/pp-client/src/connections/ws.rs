//! Clash API `/connections` WebSocket client.
//!
//! sing-box's Clash API upgrades `GET /connections` to a WebSocket when the
//! `Upgrade: websocket` header is present, then pushes the **full connection
//! snapshot** (same JSON shape as the plain HTTP response) immediately and every
//! `?interval=<ms>` (server default 1000ms). This module owns only the
//! connection handshake and message framing; snapshot diffing lives in
//! [`super::tracker`].

use std::pin::Pin;

use futures::{Stream, StreamExt};
use pp_common::{PanelError, PanelResult};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;

/// Stream of raw snapshot JSON text frames pushed by the server.
pub(crate) type SnapshotStream = Pin<Box<dyn Stream<Item = PanelResult<String>> + Send>>;

/// WebSocket connect failure classification.
#[derive(Debug)]
pub(crate) enum WsConnectError {
    /// Server did not upgrade (plain HTTP response, e.g. an implementation
    /// without WebSocket support): caller should permanently fall back to
    /// HTTP polling for this tracker session.
    Unsupported,
    /// Transient failure (refused, timeout, …): caller should retry with
    /// backoff and keep preferring WebSocket.
    Transient(PanelError),
}

/// Connect to `ws://127.0.0.1:{port}/connections?interval={interval_ms}`.
///
/// Auth: the Clash API secret is passed as the `token` query parameter (the
/// form sing-box / mihomo accept for WebSocket connections) plus a Bearer
/// header for completeness.
pub(crate) async fn connect(
    port: u16,
    secret: &str,
    interval_ms: u64,
) -> Result<SnapshotStream, WsConnectError> {
    let mut url = format!("ws://127.0.0.1:{port}/connections?interval={interval_ms}");
    if !secret.is_empty() {
        url.push_str("&token=");
        url.push_str(&urlencoding::encode(secret));
    }
    let mut request = url.into_client_request().map_err(|e| {
        WsConnectError::Transient(PanelError::Client(format!(
            "构建 Clash API WebSocket 请求失败: {e}"
        )))
    })?;
    if !secret.is_empty() {
        request.headers_mut().insert(
            "Authorization",
            format!("Bearer {secret}").parse().map_err(|e| {
                WsConnectError::Transient(PanelError::Client(format!(
                    "构建 Clash API WebSocket 认证头失败: {e}"
                )))
            })?,
        );
    }

    let (stream, _response) = match tokio_tungstenite::connect_async(request).await {
        Ok(pair) => pair,
        Err(tokio_tungstenite::tungstenite::Error::Http(response)) => {
            // Server answered with a regular HTTP response instead of the 101
            // upgrade → WebSocket unsupported on this endpoint.
            tracing::info!(
                status = ?response.status(),
                "Clash API /connections 不支持 WebSocket，回退 HTTP 轮询"
            );
            return Err(WsConnectError::Unsupported);
        }
        Err(e) => {
            return Err(WsConnectError::Transient(PanelError::Client(format!(
                "Clash API WebSocket 连接失败: {e}"
            ))));
        }
    };

    let snapshots = stream.filter_map(|message| async move {
        match message {
            Ok(Message::Text(text)) => Some(Ok(text.to_string())),
            Ok(Message::Binary(bytes)) => match String::from_utf8(bytes.to_vec()) {
                Ok(text) => Some(Ok(text)),
                Err(e) => Some(Err(PanelError::Client(format!(
                    "Clash API WebSocket 二进制帧解码失败: {e}"
                )))),
            },
            Ok(Message::Close(_)) => Some(Err(PanelError::Client(
                "Clash API WebSocket 被服务端关闭".into(),
            ))),
            Ok(_) => None, // Ping/Pong/Frame：tungstenite 自行处理
            Err(e) => Some(Err(PanelError::Client(format!(
                "Clash API WebSocket 读取失败: {e}"
            )))),
        }
    });
    Ok(Box::pin(snapshots))
}
