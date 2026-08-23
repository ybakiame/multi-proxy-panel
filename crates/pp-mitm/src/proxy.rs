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
fn websocket_connector_for(
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

/// 依据配置判断 `host` 是否应拦截。
///
/// 排除列表（`excluded_hostnames`）优先级高于白名单：命中排除的主机一律不拦截；
/// 白名单为空时除排除命中外全量拦截。
fn should_intercept_host(config: &MitmConfig, host: &str) -> bool {
    if config.excluded_hostnames.iter().any(|m| m.matches(host)) {
        return false;
    }
    if config.hostnames.is_empty() {
        return true;
    }
    config.hostnames.iter().any(|m| m.matches(host))
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
            RewriteAction::Reject => {
                let mut res = Response::new(Body::from("rejected".to_string()));
                *res.status_mut() = StatusCode::FORBIDDEN;
                return res.into();
            }
            RewriteAction::Mock {
                status,
                body,
                headers,
            } => {
                let mut res = Response::new(Body::from(body));
                *res.status_mut() =
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                // 合成响应透传自定义响应头；非法头名/值跳过。
                for (name, value) in headers {
                    match (
                        HeaderName::from_bytes(name.as_bytes()),
                        HeaderValue::from_str(&value),
                    ) {
                        (Ok(name), Ok(value)) => {
                            res.headers_mut().append(name, value);
                        }
                        _ => {
                            tracing::warn!("mock response header invalid, skipped: {name}: {value}")
                        }
                    }
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
            RewriteAction::Reject => {
                status = StatusCode::FORBIDDEN.as_u16();
                body = Some("rejected".to_string());
            }
            RewriteAction::Mock {
                status: s,
                body: b,
                headers: mock_headers,
            } => {
                status = s;
                body = Some(b);
                // mock headers 按 apply_header 语义应用到响应头：先删同名再追加。
                for (name, value) in mock_headers {
                    apply_header(&mut headers, &name, &Some(value));
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ca::CaStore;
    use crate::ca::FileCaStore;
    use crate::config::HostnameMatcher;
    use crate::recorder::MemoryRecorder;
    use crate::rewrite::{Phase, RewriteKind, RewriteRule};
    use crate::upstream::UpstreamProxy;
    use regex::Regex;
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn e2e_http_proxy_rewrites_response_and_records() {
        let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_port = upstream.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = match upstream.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let mut req = Vec::new();
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                req.extend_from_slice(&buf[..n]);
                                if req.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                        }
                    }
                    let body = r#"{"msg":"original"}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });

        // MITM 代理：白名单空（全量拦截）、响应阶段把 URL/body 中的 original 改 rewritten。
        let dir = tempdir().unwrap();
        let ca = FileCaStore::new(dir.path()).load_or_generate().unwrap();
        let recorder: Arc<dyn TrafficRecorder> = Arc::new(MemoryRecorder::new(16));
        let rewrite = RewriteEngine {
            rules: vec![RewriteRule {
                kind: RewriteKind::BodyRewrite {
                    phase: Phase::Response,
                    replacement: "rewritten".to_string(),
                },
                pattern: Regex::new("original").unwrap(),
            }],
        };
        let config = MitmConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            hostnames: Vec::<HostnameMatcher>::new(),
            record_enabled: true,
            ..MitmConfig::default()
        };
        let proxy = MitmProxy::new(config, rewrite, None, Arc::clone(&recorder), ca);
        let running = proxy.start().await.unwrap();

        // 经代理发起请求：路径带 original，使规则 pattern 同时命中 URL 与 body。
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::http(format!("http://{}", running.addr)).unwrap())
            .build()
            .unwrap();
        let resp = client
            .get(format!("http://127.0.0.1:{server_port}/original"))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let body = resp.text().await.unwrap();
        assert!(
            body.contains("rewritten"),
            "response body not rewritten: {body:?}"
        );

        let records = recorder.list();
        assert_eq!(records.len(), 1, "expected exactly one recorded exchange");
        assert!(
            records[0].url.contains("/original"),
            "unexpected recorded url: {}",
            records[0].url
        );
        assert_eq!(records[0].response_status, 200);
        assert_eq!(
            records[0].response_body.as_deref(),
            Some(r#"{"msg":"rewritten"}"#)
        );

        running.shutdown();
        server.abort();
    }

    #[tokio::test]
    async fn e2e_http_chain_routes_through_http_parent_proxy() {
        // 上游目标：本地 TCP HTTP server。
        let upstream = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let server_port = upstream.local_addr().unwrap().port();
        let server = tokio::spawn(async move {
            loop {
                let (mut socket, _) = match upstream.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let mut req = Vec::new();
                    loop {
                        match socket.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                req.extend_from_slice(&buf[..n]);
                                if req.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                        }
                    }
                    let body = r#"{"via":"upstream"}"#;
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.shutdown().await;
                });
            }
        });

        // 极简 HTTP 父代理：收到 CONNECT 后连目标、回 200，再双向转发。
        let parent = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let parent_port = parent.local_addr().unwrap().port();
        let connect_count = Arc::new(AtomicUsize::new(0));
        let count_for_server = Arc::clone(&connect_count);
        let parent_task = tokio::spawn(async move {
            loop {
                let (mut client, _) = match parent.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                let count = Arc::clone(&count_for_server);
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut chunk = [0u8; 1024];
                    loop {
                        match client.read(&mut chunk).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => {
                                buf.extend_from_slice(&chunk[..n]);
                                if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                        }
                    }
                    let request_line = String::from_utf8_lossy(&buf);
                    let target = match request_line.split_whitespace().nth(1) {
                        Some(t) if request_line.starts_with("CONNECT ") => t.to_string(),
                        _ => return,
                    };
                    count.fetch_add(1, Ordering::SeqCst);
                    let mut target_stream = match tokio::net::TcpStream::connect(&target).await {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    let _ = client
                        .write_all(b"HTTP/1.1 200 Connection established\r\n\r\n")
                        .await;
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut target_stream).await;
                });
            }
        });

        // MITM 代理：全量拦截 + upstream 指向父代理。
        let dir = tempdir().unwrap();
        let ca = FileCaStore::new(dir.path()).load_or_generate().unwrap();
        let recorder: Arc<dyn TrafficRecorder> = Arc::new(MemoryRecorder::new(16));
        let rewrite = RewriteEngine { rules: Vec::new() };
        let config = MitmConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            hostnames: Vec::<HostnameMatcher>::new(),
            record_enabled: true,
            upstream: UpstreamProxy::Http {
                addr: SocketAddr::from(([127, 0, 0, 1], parent_port)),
            },
            ..MitmConfig::default()
        };
        let proxy = MitmProxy::new(config, rewrite, None, Arc::clone(&recorder), ca);
        let running = proxy.start().await.unwrap();

        // 经 MITM 代理发请求，断言响应正确、流量确实经过父代理且被记录。
        let client = reqwest::Client::builder()
            .proxy(reqwest::Proxy::http(format!("http://{}", running.addr)).unwrap())
            .build()
            .unwrap();
        let resp = client
            .get(format!("http://127.0.0.1:{server_port}/hello"))
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        assert_eq!(resp.text().await.unwrap(), r#"{"via":"upstream"}"#);
        assert_eq!(
            connect_count.load(Ordering::SeqCst),
            1,
            "traffic must go through the parent proxy exactly once"
        );

        let records = recorder.list();
        assert_eq!(records.len(), 1, "expected exactly one recorded exchange");
        assert_eq!(records[0].response_status, 200);
        assert!(
            records[0].url.contains("/hello"),
            "unexpected recorded url: {}",
            records[0].url
        );

        running.shutdown();
        server.abort();
        parent_task.abort();
    }

    #[test]
    fn websocket_connector_wired_only_for_upstream_chain() {
        let provider = hudsucker::rustls::crypto::aws_lc_rs::default_provider();
        let http_addr: SocketAddr = "127.0.0.1:1".parse().unwrap();
        let socks5_addr: SocketAddr = "127.0.0.1:1".parse().unwrap();

        // 直连：不设置 WebSocket 连接器（保持 hudsucker 默认）。
        let direct = UpstreamConnector::new(UpstreamProxy::Direct, provider.clone()).unwrap();
        assert!(
            websocket_connector_for(UpstreamProxy::Direct, &direct).is_none(),
            "Direct 上游不应设置 WebSocket 连接器"
        );

        // HTTP 父代理：设置连接器，且 wss 仅声明 http/1.1 ALPN（WebSocket
        // 基于 HTTP/1.1 Upgrade，声明 h2 会破坏偏好 h2 上游的握手）。
        let http =
            UpstreamConnector::new(UpstreamProxy::Http { addr: http_addr }, provider.clone())
                .unwrap();
        let connector = websocket_connector_for(UpstreamProxy::Http { addr: http_addr }, &http)
            .expect("HTTP 上游应设置 WebSocket 连接器");
        match connector {
            hudsucker::tokio_tungstenite::Connector::Rustls(config) => {
                assert_eq!(
                    config.alpn_protocols,
                    vec![b"http/1.1".to_vec()],
                    "wss 仅支持 HTTP/1.1，不得声明 h2 ALPN"
                );
            }
            _ => panic!("expected rustls websocket connector"),
        }

        // SOCKS5 父代理：同样设置。
        let socks5 =
            UpstreamConnector::new(UpstreamProxy::Socks5 { addr: socks5_addr }, provider).unwrap();
        assert!(
            websocket_connector_for(UpstreamProxy::Socks5 { addr: socks5_addr }, &socks5).is_some(),
            "SOCKS5 上游应设置 WebSocket 连接器"
        );
    }

    /// 构造带指定 hostnames / excluded_hostnames 的 Handler。
    fn test_handler(
        hostnames: Vec<HostnameMatcher>,
        excluded_hostnames: Vec<HostnameMatcher>,
    ) -> Handler {
        let recorder: Arc<dyn TrafficRecorder> = Arc::new(MemoryRecorder::new(16));
        let rewrite = RewriteEngine { rules: Vec::new() };
        let config = MitmConfig {
            listen_addr: "127.0.0.1:0".parse().unwrap(),
            hostnames,
            excluded_hostnames,
            ..MitmConfig::default()
        };
        Handler {
            config: Arc::new(config),
            rewrite: Arc::new(rewrite),
            hooks: None,
            recorder,
            state: None,
        }
    }

    /// CONNECT 请求：uri 为 `host:443` 形式，返回值与
    /// `should_intercept_connect` 内部提取的 host 一致（`vip.iqiyi.com:443` → `vip.iqiyi.com`）。
    fn connect_host(host: &str) -> String {
        Request::builder()
            .uri(format!("{host}:443"))
            .body(Body::empty())
            .unwrap()
            .uri()
            .host()
            .unwrap()
            .split(':')
            .next()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn should_intercept_connect_excluded_host_wins_over_whitelist() {
        // `*.iqiyi.com` 在白名单，`vip.iqiyi.com` 同时被排除 → 排除命中时不拦截。
        let handler = test_handler(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            vec![HostnameMatcher::Suffix("vip.iqiyi.com".to_string())],
        );
        assert!(
            !should_intercept_host(&handler.config, &connect_host("vip.iqiyi.com")),
            "排除命中即使白名单命中也不应拦截"
        );
    }

    #[tokio::test]
    async fn should_intercept_connect_whitelist_hit_intercepts() {
        let handler = test_handler(
            vec![HostnameMatcher::Suffix("iqiyi.com".to_string())],
            vec![HostnameMatcher::Exact("blocked.example.com".to_string())],
        );
        assert!(
            should_intercept_host(&handler.config, &connect_host("api.iqiyi.com")),
            "未排除且白名单命中应拦截"
        );
        assert!(
            !should_intercept_host(&handler.config, &connect_host("blocked.example.com")),
            "白名单未命中不应拦截"
        );
    }

    #[tokio::test]
    async fn should_intercept_connect_empty_whitelist_intercepts_unless_excluded() {
        let handler = test_handler(
            Vec::new(),
            vec![HostnameMatcher::Suffix("vip.iqiyi.com".to_string())],
        );
        assert!(
            should_intercept_host(&handler.config, &connect_host("example.com")),
            "白名单为空时未命中排除应全量拦截"
        );
        assert!(
            !should_intercept_host(&handler.config, &connect_host("vip.iqiyi.com")),
            "白名单为空时命中排除仍不拦截"
        );
    }
}
