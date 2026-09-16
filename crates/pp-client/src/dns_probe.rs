//! DNS 服务器探测（真实 DNS 查询延迟，借鉴 karing `testDNSConnectLatency` 语义）。
//!
//! 对单个 DNS 服务器发起一次真实 `A` 记录查询并测量往返延迟：
//!
//! - `udp`：手写 wireformat 查询包经 UDP 直连（默认 53 端口）；
//! - `tls`（DoT）：TCP + rustls（webpki 根证书）建立 TLS，发送 2 字节长度前缀的
//!   wireformat 查询（RFC 7858，默认 853 端口）；
//! - `https`（DoH）：POST `application/dns-message` 到 `https://<server>/dns-query`
//!   （RFC 8484，默认 443 端口）；
//! - `quic` / `h3` / `local` / `fakeip`：v1 不支持探测，返回明确错误文案。
//!
//! 探测统一 3 秒超时；响应校验 transaction id 与 RCODE（非 NOERROR 视为失败）。
//! 服务器为域名时由系统解析（探测路径本身不走核心 DNS，避免与运行中代理互相干扰）。

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// 默认探测目标域（与 karing `kDNSTestDomain` 一致）。
pub const DEFAULT_PROBE_DOMAIN: &str = "gstatic.com";
/// 探测超时（单次查询往返）。
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// 单个 DNS 服务器探测入参（字段与切片 `DnsServer` 对齐）。
#[derive(Debug, Clone)]
pub struct DnsProbeInput {
    /// 服务器类型（`udp` / `tls` / `https` / `quic` / `h3` / `local` / `fakeip`）。
    pub server_type: String,
    /// 服务器地址（IP 或域名）。
    pub server: String,
    /// 端口（缺省按类型：udp 53 / tls 853 / https 443）。
    pub server_port: Option<u16>,
    /// 探测目标域（缺省 [`DEFAULT_PROBE_DOMAIN`]）。
    pub domain: Option<String>,
}

/// 探测结果：往返延迟 + 应答地址（A/AAAA）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsProbeResult {
    /// 往返延迟（毫秒）。
    pub latency_ms: u64,
    /// 应答中的 IP 地址（A / AAAA 记录，字符串形态）。
    pub answers: Vec<String>,
}

/// 探测入口：返回往返延迟（毫秒）或错误文案（UI 直接展示）。
pub async fn probe_dns_server(input: &DnsProbeInput) -> Result<u64, String> {
    Ok(probe_dns_server_full(input).await?.latency_ms)
}

/// 完整探测入口：往返延迟 + 应答 IP 列表（诊断工具用）。
pub async fn probe_dns_server_full(input: &DnsProbeInput) -> Result<DnsProbeResult, String> {
    let server = input.server.trim();
    if server.is_empty() {
        return Err("服务器地址为空".to_string());
    }
    let domain = input
        .domain
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .unwrap_or(DEFAULT_PROBE_DOMAIN)
        .to_string();
    let query = build_dns_query(rand::random(), &domain)?;
    match input.server_type.as_str() {
        "udp" => probe_udp(server, input.server_port.unwrap_or(53), &query).await,
        "tls" => probe_tls(server, input.server_port.unwrap_or(853), &query).await,
        "https" => probe_https(server, input.server_port.unwrap_or(443), &query).await,
        other => Err(format!("{other} 类型暂不支持探测")),
    }
}

/// 构造 wireformat 查询包（RD=1，单个 A 记录问题）。
fn build_dns_query(id: u16, domain: &str) -> Result<Vec<u8>, String> {
    let mut packet = Vec::with_capacity(64);
    packet.extend_from_slice(&id.to_be_bytes());
    packet.extend_from_slice(&0x0100u16.to_be_bytes()); // RD
    packet.extend_from_slice(&1u16.to_be_bytes()); // QDCOUNT
    packet.extend_from_slice(&0u16.to_be_bytes()); // ANCOUNT
    packet.extend_from_slice(&0u16.to_be_bytes()); // NSCOUNT
    packet.extend_from_slice(&0u16.to_be_bytes()); // ARCOUNT
    for label in domain.split('.') {
        let label = label.trim();
        if label.is_empty() || label.len() > 63 {
            return Err(format!("探测域名 `{domain}` 非法"));
        }
        packet.push(label.len() as u8);
        packet.extend_from_slice(label.as_bytes());
    }
    packet.push(0);
    packet.extend_from_slice(&1u16.to_be_bytes()); // QTYPE A
    packet.extend_from_slice(&1u16.to_be_bytes()); // QCLASS IN
    Ok(packet)
}

/// 校验响应：transaction id 一致且 RCODE = NOERROR。
fn check_dns_response(query: &[u8], response: &[u8]) -> Result<(), String> {
    if response.len() < 12 {
        return Err("响应包过短".to_string());
    }
    if response[0..2] != query[0..2] {
        return Err("响应 id 不匹配".to_string());
    }
    let rcode = response[3] & 0x0F;
    if rcode != 0 {
        return Err(format!("RCODE {rcode}"));
    }
    Ok(())
}

/// UDP 直连探测。
async fn probe_udp(server: &str, port: u16, query: &[u8]) -> Result<DnsProbeResult, String> {
    // 按目标地址族选择本地绑定地址（v6 服务器需 v6 本地 socket）。
    let bind_addr = match server.parse::<IpAddr>() {
        Ok(IpAddr::V6(_)) => "[::]:0",
        _ => "0.0.0.0:0",
    };
    let socket = tokio::net::UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| format!("UDP 绑定失败: {e}"))?;
    socket
        .connect((server, port))
        .await
        .map_err(|e| format!("UDP 连接失败: {e}"))?;
    let started = Instant::now();
    let answers = tokio::time::timeout(PROBE_TIMEOUT, async {
        socket.send(query).await.map_err(|e| e.to_string())?;
        let mut buf = [0u8; 512];
        let len = socket.recv(&mut buf).await.map_err(|e| e.to_string())?;
        check_dns_response(query, &buf[..len])?;
        Ok::<_, String>(parse_dns_answers(&buf[..len]))
    })
    .await
    .map_err(|_| "探测超时".to_string())??;
    Ok(DnsProbeResult {
        latency_ms: started.elapsed().as_millis() as u64,
        answers,
    })
}

/// DoT（RFC 7858）探测：TLS 建立后发送 2 字节长度前缀的 wireformat 查询。
async fn probe_tls(server: &str, port: u16, query: &[u8]) -> Result<DnsProbeResult, String> {
    let started = Instant::now();
    let stream = tokio::time::timeout(
        PROBE_TIMEOUT,
        tokio::net::TcpStream::connect((server, port)),
    )
    .await
    .map_err(|_| "TCP 连接超时".to_string())
    .and_then(|r| r.map_err(|e| format!("TCP 连接失败: {e}")))?;

    let server_name = match server.parse::<IpAddr>() {
        Ok(ip) => rustls::pki_types::ServerName::IpAddress(ip.into()),
        Err(_) => rustls::pki_types::ServerName::try_from(server.to_string())
            .map_err(|e| format!("服务器域名非法: {e}"))?,
    };
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let mut tls = tokio::time::timeout(PROBE_TIMEOUT, connector.connect(server_name, stream))
        .await
        .map_err(|_| "TLS 握手超时".to_string())
        .and_then(|r| r.map_err(|e| format!("TLS 握手失败: {e}")))?;

    let query_len = u16::try_from(query.len()).map_err(|_| "查询包过长".to_string())?;
    let mut framed = query_len.to_be_bytes().to_vec();
    framed.extend_from_slice(query);
    let answers = tokio::time::timeout(PROBE_TIMEOUT, async {
        tls.write_all(&framed).await.map_err(|e| e.to_string())?;
        let mut len_buf = [0u8; 2];
        tls.read_exact(&mut len_buf)
            .await
            .map_err(|e| e.to_string())?;
        let resp_len = u16::from_be_bytes(len_buf) as usize;
        if resp_len > 4096 {
            return Err("响应包过长".to_string());
        }
        let mut resp = vec![0u8; resp_len];
        tls.read_exact(&mut resp).await.map_err(|e| e.to_string())?;
        check_dns_response(query, &resp)?;
        Ok::<_, String>(parse_dns_answers(&resp))
    })
    .await
    .map_err(|_| "探测超时".to_string())??;
    Ok(DnsProbeResult {
        latency_ms: started.elapsed().as_millis() as u64,
        answers,
    })
}

/// DoH（RFC 8484）探测：POST `application/dns-message` 到 `/dns-query`。
async fn probe_https(server: &str, port: u16, query: &[u8]) -> Result<DnsProbeResult, String> {
    let url = if port == 443 {
        format!("https://{server}/dns-query")
    } else {
        format!("https://{server}:{port}/dns-query")
    };
    let client = reqwest::Client::builder()
        .timeout(PROBE_TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
    let started = Instant::now();
    let response = client
        .post(&url)
        .header("content-type", "application/dns-message")
        .header("accept", "application/dns-message")
        .body(query.to_vec())
        .send()
        .await
        .map_err(|e| format!("DoH 请求失败: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("DoH 响应 {}", response.status()));
    }
    let body = response
        .bytes()
        .await
        .map_err(|e| format!("DoH 读取响应失败: {e}"))?;
    check_dns_response(query, &body)?;
    Ok(DnsProbeResult {
        latency_ms: started.elapsed().as_millis() as u64,
        answers: parse_dns_answers(&body),
    })
}

/// 解析 wireformat 响应中的 A / AAAA 应答地址（诊断展示用）。
///
/// 依次跳过 12 字节头部与问题段（label 序列或 0xC0 压缩指针），再逐个读应答记录；
/// 仅提取 A（4 字节）/ AAAA（16 字节）RDATA，其余类型跳过。包截断 / 指针越界时返回
/// 已解析到的部分（不报错——探测成败已由 `check_dns_response` 裁定）。
pub fn parse_dns_answers(response: &[u8]) -> Vec<String> {
    let mut answers = Vec::new();
    if response.len() < 12 {
        return answers;
    }
    let qdcount = u16::from_be_bytes([response[4], response[5]]) as usize;
    let ancount = u16::from_be_bytes([response[6], response[7]]) as usize;
    let mut pos = 12;

    // 跳过名字（label 序列直到 0，或 0xC0 压缩指针两字节）。
    fn skip_name(buf: &[u8], mut pos: usize) -> Option<usize> {
        loop {
            let len = *buf.get(pos)? as usize;
            if len == 0 {
                return Some(pos + 1);
            }
            if len & 0xC0 == 0xC0 {
                return Some(pos + 2);
            }
            if len & 0xC0 != 0 {
                return None;
            }
            pos = pos.checked_add(1 + len)?;
        }
    }

    for _ in 0..qdcount {
        match skip_name(response, pos).and_then(|p| p.checked_add(4)) {
            Some(p) if p <= response.len() => pos = p,
            _ => return answers,
        }
    }
    for _ in 0..ancount {
        let Some(p) = skip_name(response, pos) else {
            break;
        };
        pos = p;
        if pos + 10 > response.len() {
            break;
        }
        let rtype = u16::from_be_bytes([response[pos], response[pos + 1]]);
        let rdlen = u16::from_be_bytes([response[pos + 8], response[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlen > response.len() {
            break;
        }
        let rdata = &response[pos..pos + rdlen];
        match (rtype, rdlen) {
            (1, 4) => answers
                .push(std::net::Ipv4Addr::new(rdata[0], rdata[1], rdata[2], rdata[3]).to_string()),
            (28, 16) => {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(rdata);
                answers.push(std::net::Ipv6Addr::from(octets).to_string());
            }
            _ => {}
        }
        pos += rdlen;
    }
    answers
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 查询包形态：id 回写、RD 置位、QDCOUNT=1、问题段以根标签 0 结尾 + A/IN。
    #[test]
    fn build_query_wireformat_shape() {
        let packet = build_dns_query(0xABCD, "gstatic.com").unwrap();
        assert_eq!(&packet[0..2], &[0xAB, 0xCD]);
        assert_eq!(&packet[2..4], &[0x01, 0x00]);
        assert_eq!(&packet[4..6], &[0x00, 0x01]);
        // 问题段：gstatic(7) com(3) 0 + qtype + qclass
        let question_end = packet.len() - 4;
        assert_eq!(packet[question_end - 1], 0, "根标签结束符");
        assert_eq!(&packet[question_end..], &[0, 1, 0, 1]);
    }

    /// 非法域名（空标签 / 超长标签）拒绝构造。
    #[test]
    fn build_query_rejects_invalid_domain() {
        assert!(build_dns_query(1, "a..com").is_err());
        assert!(build_dns_query(1, &format!("{}.com", "x".repeat(64))).is_err());
    }

    /// 响应校验：id 不一致 / RCODE 非 0 / 包过短均为错误。
    #[test]
    fn check_response_validates_id_and_rcode() {
        let query = build_dns_query(0x1234, "gstatic.com").unwrap();
        let mut ok = vec![0u8; 12];
        ok[0..2].copy_from_slice(&[0x12, 0x34]);
        ok[2] = 0x81;
        ok[3] = 0x80; // standard response, NOERROR
        check_dns_response(&query, &ok).unwrap();

        let mut bad_id = ok.clone();
        bad_id[0] = 0xFF;
        assert!(check_dns_response(&query, &bad_id).is_err());

        let mut bad_rcode = ok.clone();
        bad_rcode[3] = 0x83; // NXDOMAIN
        assert!(check_dns_response(&query, &bad_rcode).is_err());

        assert!(check_dns_response(&query, &[0u8; 4]).is_err());
    }

    /// 应答解析：构造含压缩指针 + A/AAAA/CNAME 混合应答，仅提取 A/AAAA 地址。
    #[test]
    fn parse_answers_extracts_a_and_aaaa() {
        // 手工构造：header(qd=1, an=3) + question(gstatic.com A IN)
        // + answer1(name=压缩指针 0xC00C, A, 1.2.3.4)
        // + answer2(CNAME，跳过) + answer3(AAAA, ::1)
        let mut resp = build_dns_query(0x1234, "gstatic.com").unwrap();
        resp[6] = 0;
        resp[7] = 3; // ANCOUNT=3
        let answer = |rtype: u16, rdlen: u16| {
            let mut v = vec![0xC0, 0x0C]; // 压缩指针指向问题段域名
            v.extend_from_slice(&rtype.to_be_bytes());
            v.extend_from_slice(&1u16.to_be_bytes()); // IN
            v.extend_from_slice(&60u32.to_be_bytes()); // TTL
            v.extend_from_slice(&rdlen.to_be_bytes());
            v
        };
        resp.extend_from_slice(&answer(1, 4));
        resp.extend_from_slice(&[1, 2, 3, 4]);
        resp.extend_from_slice(&answer(5, 4));
        resp.extend_from_slice(&[0xC0, 0x0C, 0xC0, 0x0C]); // 伪 CNAME 目标
        resp.extend_from_slice(&answer(28, 16));
        resp.extend_from_slice(&[0u8; 15]);
        resp.push(1); // ::1

        let answers = parse_dns_answers(&resp);
        assert_eq!(answers, vec!["1.2.3.4".to_string(), "::1".to_string()]);

        // 截断包不 panic，返回已解析部分。
        let truncated = &resp[..resp.len() - 8];
        let _ = parse_dns_answers(truncated);
    }

    /// 空服务器地址与不支持类型给出明确错误（不发包）。
    #[tokio::test]
    async fn probe_rejects_empty_server_and_unsupported_type() {
        let input = DnsProbeInput {
            server_type: "udp".to_string(),
            server: " ".to_string(),
            server_port: None,
            domain: None,
        };
        assert!(probe_dns_server(&input).await.is_err());

        let input = DnsProbeInput {
            server_type: "quic".to_string(),
            server: "dns.adguard.com".to_string(),
            server_port: None,
            domain: None,
        };
        let err = probe_dns_server(&input).await.unwrap_err();
        assert!(err.contains("暂不支持"), "{err}");
    }
}
