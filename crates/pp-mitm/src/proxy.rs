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
use crate::rewrite::{RewriteAction, RewriteEngine};
use crate::script_hook::ScriptHookEngine;
use crate::upstream::UpstreamConnector;

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

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let handler = Handler {
            config: Arc::new(self.config),
            rewrite: self.rewrite,
            hooks: self.hooks,
            recorder: self.recorder,
            state: None,
        };

        let proxy = Proxy::builder()
            .with_listener(listener)
            .with_ca(ca)
            .with_http_connector(connector)
            .with_http_handler(handler)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
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
            RewriteAction::Reject => {
                let mut res = Response::new(Body::from("rejected".to_string()));
                *res.status_mut() = StatusCode::FORBIDDEN;
                return res.into();
            }
            RewriteAction::Mock { status, body } => {
                let mut res = Response::new(Body::from(body));
                *res.status_mut() =
                    StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                return res.into();
            }
            RewriteAction::Continue => {}
        }

        if let Some(hooks) = &self.hooks {
            run_request_hooks_send(
                Arc::clone(hooks),
                url.clone(),
                method.clone(),
                &mut headers,
                &mut body,
            )
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
            RewriteAction::Mock { status: s, body: b } => {
                status = s;
                body = Some(b);
            }
            RewriteAction::Continue => {}
        }

        if let Some(hooks) = &self.hooks {
            run_response_hooks_send(
                Arc::clone(hooks),
                url.clone(),
                &mut status,
                &mut headers,
                &mut body,
            )
            .await;
        }

        if self.config.record_enabled {
            if let Some(state) = state {
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
        // CONNECT 请求的 URI 形如 host:port，去掉端口后与白名单匹配。
        let host = req
            .uri()
            .host()
            .unwrap_or_default()
            .split(':')
            .next()
            .unwrap_or_default();
        if self.config.hostnames.is_empty() {
            return true;
        }
        self.config.hostnames.iter().any(|m| m.matches(host))
    }
}

/// 在独立阻塞线程的 current_thread 运行时中执行请求阶段脚本钩子。
///
/// QuickJS 引擎的 future 不是 `Send`，无法直接在 hudsucker 的 `+ Send`
/// handler 中 `.await`，因此把数据搬进 `spawn_blocking` 线程后在其内部新建
/// 单线程运行时执行，结束后把改写结果搬回。
async fn run_request_hooks_send(
    hooks: Arc<ScriptHookEngine>,
    url: String,
    method: String,
    headers: &mut Vec<(String, String)>,
    body: &mut Option<String>,
) {
    let mut out_headers = headers.clone();
    let mut out_body = body.clone();
    let out = tokio::task::spawn_blocking(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::warn!("init script hook runtime: {e}");
                return (out_headers, out_body);
            }
        };
        rt.block_on(hooks.run_request_hooks(&url, &method, &mut out_headers, &mut out_body));
        (out_headers, out_body)
    })
    .await;
    match out {
        Ok((h, b)) => {
            *headers = h;
            *body = b;
        }
        Err(e) => {
            tracing::warn!("script hook task failed: {e}");
        }
    }
}

/// 响应阶段脚本钩子的 `Send` 安全包装，机制同 [`run_request_hooks_send`]。
async fn run_response_hooks_send(
    hooks: Arc<ScriptHookEngine>,
    url: String,
    status: &mut u16,
    headers: &mut Vec<(String, String)>,
    body: &mut Option<String>,
) {
    let mut out_status = *status;
    let mut out_headers = headers.clone();
    let mut out_body = body.clone();
    let out = tokio::task::spawn_blocking(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::warn!("init script hook runtime: {e}");
                return (out_status, out_headers, out_body);
            }
        };
        rt.block_on(hooks.run_response_hooks(
            &url,
            &mut out_status,
            &mut out_headers,
            &mut out_body,
        ));
        (out_status, out_headers, out_body)
    })
    .await;
    match out {
        Ok((s, h, b)) => {
            *status = s;
            *headers = h;
            *body = b;
        }
        Err(e) => {
            tracing::warn!("script hook task failed: {e}");
        }
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
}
