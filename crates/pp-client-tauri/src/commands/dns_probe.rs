//! DNS 服务器探测命令（单次真实查询延迟）。
//!
//! 前端 DNS 服务器列表对启用中的服务器并发调用本命令（每服务器一次 invoke），
//! 行内展示往返毫秒数或错误文案。纯函数主体 [`probe`] 可脱离 Tauri 运行时单测。

use pp_client::dns_probe::{DnsProbeInput, probe_dns_server};
use serde::Deserialize;

/// 探测入参（与切片 `DnsServer` 字段对齐；`domain` 缺省 gstatic.com）。
#[derive(Debug, Clone, Deserialize)]
pub struct DnsServerProbeInput {
    /// 服务器类型（`udp` / `tls` / `https` / `quic` / `h3` / `local` / `fakeip`）。
    pub server_type: String,
    /// 服务器地址（IP 或域名）。
    pub server: String,
    /// 端口（缺省按类型：udp 53 / tls 853 / https 443）。
    pub server_port: Option<u16>,
    /// 探测目标域（缺省 `gstatic.com`）。
    pub domain: Option<String>,
}

// ---------------------------------------------------------------------------
// Pure command bodies (unit-testable without a Tauri runtime)
// ---------------------------------------------------------------------------

/// 执行探测：返回往返延迟（毫秒）或错误文案。
pub(crate) async fn probe(input: DnsServerProbeInput) -> Result<u64, String> {
    probe_dns_server(&DnsProbeInput {
        server_type: input.server_type,
        server: input.server,
        server_port: input.server_port,
        domain: input.domain,
    })
    .await
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// 探测单个 DNS 服务器（一次真实 A 记录查询的往返毫秒数；错误以字符串返回）。
#[tauri::command]
pub async fn dns_server_probe(input: DnsServerProbeInput) -> Result<u64, String> {
    probe(input).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 不支持类型（local / fakeip / quic / h3）与空地址的入参校验贯穿命令主体。
    #[tokio::test]
    async fn probe_rejects_unsupported_type_and_empty_server() {
        let err = probe(DnsServerProbeInput {
            server_type: "local".to_string(),
            server: String::new(),
            server_port: None,
            domain: None,
        })
        .await
        .unwrap_err();
        assert!(err.contains("为空"), "{err}");

        let err = probe(DnsServerProbeInput {
            server_type: "h3".to_string(),
            server: "8.8.8.8".to_string(),
            server_port: None,
            domain: None,
        })
        .await
        .unwrap_err();
        assert!(err.contains("暂不支持"), "{err}");
    }
}
