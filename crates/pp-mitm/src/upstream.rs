//! 上游代理链：MITM 解密后的流量经父代理转发的连接器。
//!
//! [`UpstreamProxy`] 描述上游去向（直连 / HTTP CONNECT 父代理 / SOCKS5
//! 父代理）。hudsucker 通过 `with_http_connector` 暴露 connector 扩展点
//! （要求实现 `hyper_util::client::legacy::connect::Connect`，该 trait 对
//! `tower::Service<http::Uri>` 有 blanket impl），因此这里实现一个
//! [`tower::Service<Uri>`]：收到目标 URI 后按上游策略建立 TCP 隧道
//! （HTTP CONNECT 或 SOCKS5 握手），需要时再叠加 rustls 客户端握手
//! （webpki roots 校验目标证书），最终把满足 hyper 传输要求的连接交给
//! hudsucker 的客户端发请求。

use std::future::Future;
use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use http::Uri;
use hyper::rt::ReadBufCursor;
use hyper_util::client::legacy::connect::{Connected, Connection};
use hyper_util::rt::TokioIo;
use rustls::pki_types::ServerName;
use rustls::{ClientConfig, crypto::CryptoProvider};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

/// 上游去向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum UpstreamProxy {
    /// 直连上游（默认，等价于 hudsucker 原生的 rustls + webpki roots）。
    #[default]
    Direct,
    /// 经本地 HTTP 父代理转发（CONNECT 隧道）。
    Http { addr: std::net::SocketAddr },
    /// 经本地 SOCKS5 父代理转发（无认证 CONNECT）。
    Socks5 { addr: std::net::SocketAddr },
}

/// 建立上游连接过程中的错误。
#[derive(Debug, Error)]
pub enum UpstreamError {
    /// 目标 URI 缺少可用的 authority。
    #[error("invalid upstream target uri: {0}")]
    InvalidTarget(String),
    /// 底层 TCP 连接失败。
    #[error("connect failed: {0}")]
    Connect(#[from] io::Error),
    /// 父代理握手（HTTP CONNECT / SOCKS5）失败。
    #[error("proxy handshake failed: {0}")]
    Proxy(String),
    /// TLS 握手失败。
    #[error("tls handshake failed: {0}")]
    Tls(String),
    /// 目标主机名无法构造 TLS SNI。
    #[error("invalid server name: {0}")]
    InvalidServerName(String),
}

/// 供 hudsucker/hyper 使用的自定义连接器。
///
/// 实现 `tower::Service<Uri>`，`Response` 为满足 hyper 传输要求的
/// [`UpstreamConnection`]，从而通过 hudsucker `with_http_connector` 接入。
#[derive(Clone)]
pub struct UpstreamConnector {
    upstream: UpstreamProxy,
    tls: Arc<ClientConfig>,
}

impl UpstreamConnector {
    /// 构造连接器。
    ///
    /// `provider` 与 hudsucker 原生的 `with_rustls_connector` 一致，通常传
    /// `hudsucker::rustls::crypto::aws_lc_rs::default_provider()`。
    pub fn new(upstream: UpstreamProxy, provider: CryptoProvider) -> Result<Self, UpstreamError> {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        let mut config = ClientConfig::builder_with_provider(Arc::new(provider))
            .with_safe_default_protocol_versions()
            .map_err(|e| UpstreamError::Tls(e.to_string()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        // 与 hyper-rustls 默认一致：向上游协商 HTTP/1.1 与 h2。
        config.alpn_protocols = vec![b"http/1.1".to_vec(), b"h2".to_vec()];
        Ok(Self {
            upstream,
            tls: Arc::new(config),
        })
    }
}

impl tower::Service<Uri> for UpstreamConnector {
    type Response = UpstreamConnection;
    type Error = UpstreamError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, dst: Uri) -> Self::Future {
        let upstream = self.upstream;
        let tls = Arc::clone(&self.tls);
        Box::pin(async move { connect_upstream(upstream, tls, &dst).await })
    }
}

/// 交给 hyper 的连接传输：父代理隧道（可选 TLS 层）。
#[derive(Debug)]
pub struct UpstreamConnection {
    inner: UpstreamIo,
}

#[derive(Debug)]
enum UpstreamIo {
    Plain(TokioIo<TcpStream>),
    Tls(Box<TokioIo<TlsStream<TcpStream>>>),
}

impl UpstreamConnection {
    fn new_plain(stream: TcpStream) -> Self {
        Self {
            inner: UpstreamIo::Plain(TokioIo::new(stream)),
        }
    }

    fn new_tls(stream: TlsStream<TcpStream>) -> Self {
        Self {
            inner: UpstreamIo::Tls(Box::new(TokioIo::new(stream))),
        }
    }
}

impl hyper::rt::Read for UpstreamConnection {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<Result<(), io::Error>> {
        match &mut self.get_mut().inner {
            UpstreamIo::Plain(s) => Pin::new(s).poll_read(cx, buf),
            UpstreamIo::Tls(s) => Pin::new(&mut **s).poll_read(cx, buf),
        }
    }
}

impl hyper::rt::Write for UpstreamConnection {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        match &mut self.get_mut().inner {
            UpstreamIo::Plain(s) => Pin::new(s).poll_write(cx, buf),
            UpstreamIo::Tls(s) => Pin::new(&mut **s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match &mut self.get_mut().inner {
            UpstreamIo::Plain(s) => Pin::new(s).poll_flush(cx),
            UpstreamIo::Tls(s) => Pin::new(&mut **s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        match &mut self.get_mut().inner {
            UpstreamIo::Plain(s) => Pin::new(s).poll_shutdown(cx),
            UpstreamIo::Tls(s) => Pin::new(&mut **s).poll_shutdown(cx),
        }
    }
}

impl Connection for UpstreamConnection {
    fn connected(&self) -> Connected {
        Connected::new()
    }
}

/// 按上游策略建立到目标的连接并返回 hyper 传输。
async fn connect_upstream(
    upstream: UpstreamProxy,
    tls: Arc<ClientConfig>,
    dst: &Uri,
) -> Result<UpstreamConnection, UpstreamError> {
    let (host, port, use_tls) = parse_target(dst)?;
    let tcp = connect_tcp(upstream, &host, port).await?;
    if use_tls {
        let stream = tls_connect(tcp, tls, &host).await?;
        Ok(UpstreamConnection::new_tls(stream))
    } else {
        Ok(UpstreamConnection::new_plain(tcp))
    }
}

/// 从目标 URI 提取 host、port 与是否需要 TLS。
fn parse_target(dst: &Uri) -> Result<(String, u16, bool), UpstreamError> {
    let authority = dst
        .authority()
        .ok_or_else(|| UpstreamError::InvalidTarget(dst.to_string()))?;
    let use_tls = dst.scheme_str() == Some("https");
    let default_port = if use_tls { 443 } else { 80 };
    let port = authority.port_u16().unwrap_or(default_port);
    Ok((authority.host().to_string(), port, use_tls))
}

/// 建立到目标的 TCP 连接：直连或经父代理隧道。
async fn connect_tcp(
    upstream: UpstreamProxy,
    host: &str,
    port: u16,
) -> Result<TcpStream, UpstreamError> {
    match upstream {
        UpstreamProxy::Direct => Ok(TcpStream::connect((host, port)).await?),
        UpstreamProxy::Http { addr } => {
            let stream = TcpStream::connect(addr).await?;
            http_connect(stream, host, port).await
        }
        UpstreamProxy::Socks5 { addr } => {
            let stream = TcpStream::connect(addr).await?;
            socks5_connect(stream, host, port).await
        }
    }
}

/// 经 HTTP 父代理建立 CONNECT 隧道。
async fn http_connect(
    mut stream: TcpStream,
    host: &str,
    port: u16,
) -> Result<TcpStream, UpstreamError> {
    let target = format!("{host}:{port}");
    let request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n\r\n");
    stream.write_all(request.as_bytes()).await?;
    stream.flush().await?;

    let mut buf = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Err(UpstreamError::Proxy(format!(
                "CONNECT {target}: parent proxy closed connection"
            )));
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            if pos + 4 != buf.len() {
                return Err(UpstreamError::Proxy(format!(
                    "CONNECT {target}: unexpected data after response"
                )));
            }
            let head = String::from_utf8_lossy(&buf[..pos]);
            let status = head.split_whitespace().nth(1).unwrap_or_default();
            if status != "200" {
                return Err(UpstreamError::Proxy(format!(
                    "CONNECT {target} failed: {head}"
                )));
            }
            return Ok(stream);
        }
        if buf.len() > 16 * 1024 {
            return Err(UpstreamError::Proxy(format!(
                "CONNECT {target}: response header too large"
            )));
        }
    }
}

/// 经 SOCKS5 父代理建立 CONNECT 隧道（无认证、域名寻址）。
async fn socks5_connect(
    mut stream: TcpStream,
    host: &str,
    port: u16,
) -> Result<TcpStream, UpstreamError> {
    // 问候：版本 5、一种认证方法（无认证）。
    stream.write_all(&[0x05, 0x01, 0x00]).await?;
    stream.flush().await?;

    let mut greeting = [0u8; 2];
    stream.read_exact(&mut greeting).await?;
    if greeting != [0x05, 0x00] {
        return Err(UpstreamError::Proxy(format!(
            "socks5 greeting failed: ver={} method={}",
            greeting[0], greeting[1]
        )));
    }

    if host.len() > 255 {
        return Err(UpstreamError::Proxy("socks5 target host too long".into()));
    }
    // CONNECT 请求：ATYP=0x03（域名）。
    let mut request = Vec::with_capacity(7 + host.len());
    request.extend_from_slice(&[0x05, 0x01, 0x00, 0x03]);
    request.push(host.len() as u8);
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&request).await?;
    stream.flush().await?;

    // 响应头：VER REP RSV ATYP。
    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[0] != 0x05 || head[1] != 0x00 {
        return Err(UpstreamError::Proxy(format!(
            "socks5 connect failed: ver={} rep={}",
            head[0], head[1]
        )));
    }
    // 丢弃父代理的绑定地址。
    match head[3] {
        0x01 => {
            let mut addr = [0u8; 4];
            stream.read_exact(&mut addr).await?;
        }
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            let mut addr = vec![0u8; len[0] as usize];
            stream.read_exact(&mut addr).await?;
        }
        0x04 => {
            let mut addr = [0u8; 16];
            stream.read_exact(&mut addr).await?;
        }
        atyp => {
            return Err(UpstreamError::Proxy(format!(
                "socks5 unsupported bind address type: {atyp}"
            )));
        }
    }
    let mut port_buf = [0u8; 2];
    stream.read_exact(&mut port_buf).await?;
    Ok(stream)
}

/// 在已建立的隧道/直连上执行 rustls 客户端握手（webpki roots 校验）。
async fn tls_connect(
    stream: TcpStream,
    tls: Arc<ClientConfig>,
    host: &str,
) -> Result<TlsStream<TcpStream>, UpstreamError> {
    let server_name = server_name(host)?;
    let connector = tokio_rustls::TlsConnector::from(tls);
    connector
        .connect(server_name, stream)
        .await
        .map_err(|e| UpstreamError::Tls(e.to_string()))
}

/// 从主机名构造 TLS SNI：IP 走 `IpAddress`，域名走 `DnsName`。
fn server_name(host: &str) -> Result<ServerName<'static>, UpstreamError> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        Ok(ServerName::IpAddress(ip.into()))
    } else {
        ServerName::try_from(host.to_string())
            .map_err(|_| UpstreamError::InvalidServerName(host.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::net::TcpListener;

    #[test]
    fn upstream_proxy_defaults_to_direct_and_roundtrips_json() {
        assert_eq!(UpstreamProxy::default(), UpstreamProxy::Direct);

        let direct = UpstreamProxy::Direct;
        let json = serde_json::to_string(&direct).unwrap();
        assert_eq!(json, "\"Direct\"");
        assert_eq!(
            serde_json::from_str::<UpstreamProxy>(&json).unwrap(),
            direct
        );

        let http = UpstreamProxy::Http {
            addr: "127.0.0.1:7890".parse().unwrap(),
        };
        let json = serde_json::to_string(&http).unwrap();
        assert_eq!(json, r#"{"Http":{"addr":"127.0.0.1:7890"}}"#);
        assert_eq!(serde_json::from_str::<UpstreamProxy>(&json).unwrap(), http);

        let socks5 = UpstreamProxy::Socks5 {
            addr: "[::1]:7891".parse().unwrap(),
        };
        let json = serde_json::to_string(&socks5).unwrap();
        assert_eq!(json, r#"{"Socks5":{"addr":"[::1]:7891"}}"#);
        assert_eq!(
            serde_json::from_str::<UpstreamProxy>(&json).unwrap(),
            socks5
        );
    }

    #[tokio::test]
    async fn socks5_connect_establishes_tunnel_through_parent() {
        // 上游目标：只回显一次。
        let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let target_port = upstream.local_addr().unwrap().port();
        let upstream_task = tokio::spawn(async move {
            let (mut socket, _) = upstream.accept().await.unwrap();
            let mut buf = [0u8; 64];
            let n = socket.read(&mut buf).await.unwrap();
            let _ = socket.write_all(&buf[..n]).await;
        });

        // 极简 SOCKS5 父代理：无认证 + 域名 CONNECT + 双向转发。
        let parent = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let parent_addr = parent.local_addr().unwrap();
        let connects = Arc::new(AtomicUsize::new(0));
        let connects_for_server = Arc::clone(&connects);
        let parent_task = tokio::spawn(async move {
            loop {
                let (mut client, _) = match parent.accept().await {
                    Ok(pair) => pair,
                    Err(_) => break,
                };
                let connects = Arc::clone(&connects_for_server);
                tokio::spawn(async move {
                    let mut greeting = [0u8; 3];
                    if client.read_exact(&mut greeting).await.is_err()
                        || greeting != [0x05, 0x01, 0x00]
                    {
                        return;
                    }
                    let _ = client.write_all(&[0x05, 0x00]).await;
                    let mut head = [0u8; 4];
                    if client.read_exact(&mut head).await.is_err()
                        || head[..3] != [0x05, 0x01, 0x00]
                        || head[3] != 0x03
                    {
                        return;
                    }
                    let mut len = [0u8; 1];
                    if client.read_exact(&mut len).await.is_err() {
                        return;
                    }
                    let mut host = vec![0u8; len[0] as usize];
                    if client.read_exact(&mut host).await.is_err() {
                        return;
                    }
                    let mut port_buf = [0u8; 2];
                    if client.read_exact(&mut port_buf).await.is_err() {
                        return;
                    }
                    let host = String::from_utf8_lossy(&host).into_owned();
                    let port = u16::from_be_bytes(port_buf);
                    let mut target = match TcpStream::connect((host.as_str(), port)).await {
                        Ok(s) => s,
                        Err(_) => return,
                    };
                    connects.fetch_add(1, Ordering::SeqCst);
                    let _ = client
                        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                        .await;
                    let _ = tokio::io::copy_bidirectional(&mut client, &mut target).await;
                });
            }
        });

        let stream = TcpStream::connect(parent_addr).await.unwrap();
        let mut tunnel = socks5_connect(stream, "127.0.0.1", target_port)
            .await
            .unwrap();
        tunnel.write_all(b"ping").await.unwrap();
        let mut echo = [0u8; 4];
        tunnel.read_exact(&mut echo).await.unwrap();
        assert_eq!(&echo, b"ping");
        assert_eq!(
            connects.load(Ordering::SeqCst),
            1,
            "parent proxy should handle exactly one CONNECT"
        );

        upstream_task.abort();
        parent_task.abort();
    }
}
