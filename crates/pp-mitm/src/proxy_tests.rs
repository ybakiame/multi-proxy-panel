//! `proxy` 模块单元测试。

use super::*;
use crate::ca::CaStore;
use crate::ca::FileCaStore;
use crate::config::HostnameMatcher;
use crate::recorder::MemoryRecorder;
use crate::rewrite::{Phase, RewriteKind, RewriteRule};
use crate::upstream::UpstreamProxy;
use regex::Regex;
use std::net::SocketAddr;
use std::sync::Arc;
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
                body_pattern: None,
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
        UpstreamConnector::new(UpstreamProxy::Http { addr: http_addr }, provider.clone()).unwrap();
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
