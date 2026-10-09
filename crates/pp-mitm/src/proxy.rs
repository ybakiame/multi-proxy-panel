//! hudsucker 拦截代理与端到端管线。
//!
//! 基于 [hudsucker] 实现本地 MITM 代理：请求/响应依次经过重写引擎
//! （[`RewriteEngine`]）与可选脚本钩子（[`ScriptHookEngine`]）后转发，
//! 命中 [`MitmConfig::record_enabled`] 时把整条交换写入 [`TrafficRecorder`]。

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;

use chrono::Utc;
use http::header::{CONTENT_LENGTH, HeaderName, TRANSFER_ENCODING};
use http::{HeaderMap, HeaderValue, Request, Response, StatusCode};
use http_body_util::BodyExt;
use hudsucker::certificate_authority::RcgenAuthority;
use hudsucker::rcgen::{Issuer, KeyPair};
use hudsucker::{Body, HttpContext, HttpHandler, Proxy, RequestOrResponse};
use pp_common::error::{PanelError, PanelResult};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::ca::CaMaterial;
use crate::config::MitmConfig;
use crate::intercept::should_intercept_host;
use crate::recorder::{TrafficRecord, TrafficRecorder};
use crate::rewrite::{RewriteAction, RewriteEngine, apply_header};
use crate::script_hook::ScriptHookEngine;
use crate::upstream::{UpstreamConnector, UpstreamProxy};

/// 依据上游策略决定 WebSocket 连接器。
///
/// 直连（[`UpstreamProxy::Direct`]）时返回 `None`，保持 hudsucker 默认行为；
/// 经 HTTP/SOCKS5 父代理时返回基于主连接器 TLS 配置的
/// `tokio_tungstenite::Connector`，使 wss 的 TLS 层与主连接器一致。
///
/// # 已知限制
///
/// hudsucker 0.25 的 WebSocket 上游连接由 tokio-tungstenite 内部直连
/// `TcpStream` 建立（`tokio-tungstenite/src/connect.rs` 硬编码
/// `TcpStream::connect`），`with_websocket_connector` 仅控制其上的 TLS 层。
/// 因此即使设置了连接器，WebSocket 的 TCP 连接仍无法经父代理转发，这里只能
/// 做到 TLS 配置对齐；若需完整支持须 fork hudsucker 替换连接建立逻辑。
pub(crate) fn websocket_connector_for(
    upstream: UpstreamProxy,
    connector: &UpstreamConnector,
) -> Option<hudsucker::tokio_tungstenite::Connector> {
    match upstream {
        UpstreamProxy::Direct => None,
        _ => Some(hudsucker::tokio_tungstenite::Connector::Rustls(
            connector.websocket_tls_config(),
        )),
    }
}

/// hudsucker 拦截代理。
pub struct MitmProxy {
    config: MitmConfig,
    rewrite: Arc<RewriteEngine>,
    hooks: Option<Arc<ScriptHookEngine>>,
    recorder: Arc<dyn TrafficRecorder>,
    ca: CaMaterial,
}

impl MitmProxy {
    /// 构造拦截代理。
    pub fn new(
        config: MitmConfig,
        rewrite: RewriteEngine,
        hooks: Option<ScriptHookEngine>,
        recorder: Arc<dyn TrafficRecorder>,
        ca: CaMaterial,
    ) -> Self {
        Self {
            config,
            rewrite: Arc::new(rewrite),
            hooks: hooks.map(Arc::new),
            recorder,
            ca,
        }
    }

    /// 启动代理：绑定监听地址、装载 CA 并在后台运行，返回运行句柄。
    pub async fn start(self) -> PanelResult<RunningProxy> {
        let listener = TcpListener::bind(self.config.listen_addr)
            .await
            .map_err(|e| {
                PanelError::Mitm(format!(
                    "bind proxy listener {:?}: {e}",
                    self.config.listen_addr
                ))
            })?;
        let addr = listener
            .local_addr()
            .map_err(|e| PanelError::Mitm(format!("resolve proxy addr: {e}")))?;

        let provider = hudsucker::rustls::crypto::aws_lc_rs::default_provider();
        let key_pair = KeyPair::from_pem(&self.ca.key_pem)
            .map_err(|e| PanelError::Mitm(format!("parse ca key: {e}")))?;
        let issuer = Issuer::from_ca_cert_pem(&self.ca.cert_pem, key_pair)
            .map_err(|e| PanelError::Mitm(format!("parse ca cert: {e}")))?;
        let ca = RcgenAuthority::new(issuer, 1024, provider.clone());

        // 按上游去向构造自定义 connector：直连（rustls + webpki roots）或
        // 经 HTTP CONNECT / SOCKS5 父代理建隧道后 TLS 握手。
        let connector = UpstreamConnector::new(self.config.upstream, provider)
            .map_err(|e| PanelError::Mitm(format!("init upstream connector: {e}")))?;
        let websocket_connector = websocket_connector_for(self.config.upstream, &connector);
        if websocket_connector.is_some() {
            tracing::warn!(
                "WebSocket 上游仍由 hudsucker 直连（TCP 层不可插拔），仅 TLS 配置与主连接器对齐"
            );
        }

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let handler = Handler {
            config: Arc::new(self.config),
            rewrite: self.rewrite,
            hooks: self.hooks,
            recorder: self.recorder,
            state: None,
        };

        let mut builder = Proxy::builder()
            .with_listener(listener)
            .with_ca(ca)
            .with_http_connector(connector)
            .with_http_handler(handler)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            });
        if let Some(websocket_connector) = websocket_connector {
            builder = builder.with_websocket_connector(websocket_connector);
        }
        let proxy = builder
            .build()
            .map_err(|e| PanelError::Mitm(format!("build hudsucker proxy: {e}")))?;

        tokio::spawn(async move {
            if let Err(e) = proxy.start().await {
                tracing::error!("mitm proxy terminated: {e}");
            }
        });

        Ok(RunningProxy {
            addr,
            shutdown: shutdown_tx,
        })
    }
}

/// 运行中的代理实例。
pub struct RunningProxy {
    /// 实际监听地址（`listen_addr: 0` 时为本机随机端口）。
    pub addr: SocketAddr,
    shutdown: oneshot::Sender<()>,
}

impl RunningProxy {
    /// 发送优雅关闭信号，等待在途连接结束后停止。
    pub fn shutdown(self) {
        let _ = self.shutdown.send(());
    }
}

/// 请求阶段上下文，跨 handle_request → handle_response 传递。
#[derive(Debug, Clone)]
struct RequestState {
    method: String,
    url: String,
    req_headers: Vec<(String, String)>,
    req_body: Option<String>,
    start: Instant,
}

/// hudsucker 处理器：每请求独立克隆，内部 state 串联请求/响应两阶段。
#[derive(Clone)]
struct Handler {
    config: Arc<MitmConfig>,
    rewrite: Arc<RewriteEngine>,
    hooks: Option<Arc<ScriptHookEngine>>,
    recorder: Arc<dyn TrafficRecorder>,
    state: Option<RequestState>,
}

impl HttpHandler for Handler {
    async fn handle_request(
        &mut self,
        _ctx: &HttpContext,
        req: Request<Body>,
    ) -> RequestOrResponse {
        let (mut parts, body) = req.into_parts();
        let (collected, rebuilt) = collect_body(body, self.config.max_body_size).await;

        let method = parts.method.as_str().to_string();
        let url = parts.uri.to_string();
        let req_headers = headers_to_vec(&parts.headers);

        self.state = Some(RequestState {
            method: method.clone(),
            url: url.clone(),
            req_headers: req_headers.clone(),
            req_body: collected.clone(),
            start: Instant::now(),
        });

        let mut url = url;
        let mut headers = req_headers;
        let mut body = collected;

        match self
            .rewrite
            .apply_request(&mut url, &mut headers, &mut body)
        {
            RewriteAction::Mock {
                status,
                body,
                headers,
            } => {
                return mock_response(status, body, headers).into();
            }
            RewriteAction::Redirect { status, location } => {
                let mut res = mock_response(status, Vec::new(), Vec::new());
                if let Ok(value) = HeaderValue::from_str(&location) {
                    res.headers_mut().insert(http::header::LOCATION, value);
                }
                return res.into();
            }
            RewriteAction::Continue => {}
        }

        if let Some(hooks) = &self.hooks {
            hooks
                .run_request_hooks(&url, &method, &mut headers, &mut body)
                .await;
        }

        parts.headers = vec_to_headers(&headers);
        let body = body.map(Body::from).unwrap_or(rebuilt);
        Request::from_parts(parts, body).into()
    }

    async fn handle_response(&mut self, _ctx: &HttpContext, res: Response<Body>) -> Response<Body> {
        let (mut parts, body) = res.into_parts();
        let (collected, rebuilt) = collect_body(body, self.config.max_body_size).await;

        let state = self.state.take();
        let url = state.as_ref().map(|s| s.url.clone()).unwrap_or_default();

        let mut headers = headers_to_vec(&parts.headers);
        let mut body = collected;
        let mut status = parts.status.as_u16();

        match self
            .rewrite
            .apply_response(&url, &mut status, &mut headers, &mut body)
        {
            RewriteAction::Mock {
                status: s,
                body: b,
                headers: mock_headers,
            } => {
                status = s;
                body = Some(String::from_utf8_lossy(&b).into_owned());
                // mock headers 按 apply_header 语义应用到响应头：先删同名再追加。
                for (name, value) in mock_headers {
                    apply_header(&mut headers, &name, &Some(value));
                }
            }
            RewriteAction::Redirect {
                status: s,
                location,
            } => {
                status = s;
                body = Some(String::new());
                apply_header(&mut headers, "Location", &Some(location));
            }
            RewriteAction::Continue => {}
        }

        if let Some(hooks) = &self.hooks {
            hooks
                .run_response_hooks(&url, &mut status, &mut headers, &mut body)
                .await;
        }

        if self.config.record_enabled
            && let Some(state) = state
        {
            let duration_ms = state.start.elapsed().as_millis() as u64;
            self.recorder.record(TrafficRecord {
                id: Uuid::new_v4(),
                method: state.method,
                url: state.url,
                request_headers: state.req_headers,
                request_body: state.req_body,
                response_status: status,
                response_headers: headers.clone(),
                response_body: body.clone(),
                timestamp: Utc::now(),
                duration_ms,
            });
        }

        parts.status = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        parts.headers = vec_to_headers(&headers);
        // body 可能被改写，丢弃旧的长度信息，交由 hyper 按实际 body 重新分帧。
        parts.headers.remove(CONTENT_LENGTH);
        parts.headers.remove(TRANSFER_ENCODING);
        let body = body.map(Body::from).unwrap_or(rebuilt);
        Response::from_parts(parts, body)
    }

    async fn should_intercept_connect(&mut self, _ctx: &HttpContext, req: &Request<Body>) -> bool {
        // CONNECT 请求的 URI 形如 host:port，去掉端口后与匹配器比对。
        let host = req
            .uri()
            .host()
            .unwrap_or_default()
            .split(':')
            .next()
            .unwrap_or_default();
        should_intercept_host(&self.config, host)
    }

    async fn should_intercept_tls(
        &mut self,
        _ctx: &HttpContext,
        client_hello: tokio_rustls::rustls::server::ClientHello<'_>,
    ) -> bool {
        // SNI 第二道闸：CONNECT 阶段判定放行的连接仍按 ClientHello 的 SNI 复核一次，
        // 不命中白名单则返回 false，hudsucker 走盲隧道透传（internal.rs 的
        // copy_bidirectional 分支）。无 SNI 时按空主机名处理：白名单为空时仍全拦截
        // （库语义），白名单非空时不拦截。
        should_intercept_host(&self.config, client_hello.server_name().unwrap_or_default())
    }
}

/// 收集 body：超过 `max_size` 时仍整体转发但不再缓存文本。
async fn collect_body(body: Body, max_size: usize) -> (Option<String>, Body) {
    match BodyExt::collect(body).await {
        Ok(collected) => {
            let bytes = collected.to_bytes();
            let stored = if bytes.len() <= max_size {
                Some(String::from_utf8_lossy(&bytes).into_owned())
            } else {
                None
            };
            (stored, Body::from(bytes.to_vec()))
        }
        Err(e) => {
            tracing::warn!("collect body failed: {e}");
            (None, Body::from(Vec::new()))
        }
    }
}

/// 合成响应：状态码 + body + 自定义头（非法头名/值跳过）。
fn mock_response(status: u16, body: Vec<u8>, headers: Vec<(String, String)>) -> Response<Body> {
    let mut res = Response::new(Body::from(body));
    *res.status_mut() = StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    for (name, value) in headers {
        match (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            (Ok(name), Ok(value)) => {
                res.headers_mut().append(name, value);
            }
            _ => tracing::warn!("mock response header invalid, skipped: {name}: {value}"),
        }
    }
    res
}

/// hyper HeaderMap → (name, value) 列表。
fn headers_to_vec(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// (name, value) 列表 → hyper HeaderMap；非法项跳过。
fn vec_to_headers(headers: &[(String, String)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (name, value) in headers {
        if let (Ok(name), Ok(value)) = (
            HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            map.append(name, value);
        }
    }
    map
}
