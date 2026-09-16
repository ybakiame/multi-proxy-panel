//! 连通性诊断（开发者工具）：对单个域名做分步诊断，定位「开代理后断连」类问题。
//!
//! 比 karing 的 DNS 探测更进一步——不只是 DNS 服务器延迟，而是沿流量路径分层检查：
//!
//! 1. `system-dns`：系统解析器直连解析（基线；失败说明设备直连 DNS 已挂）；
//! 2. `direct-tcp`：直连 TCP 443（被墙域名预期失败，仅作对照基线）；
//! 3. `dns-servers`：当前生效的每个 DNS 服务器真实查询（延迟 + 应答 IP；
//!    生效集合 = DNS 切片 takeover 时的启用服务器，否则内置默认 local/remote）；
//! 4. `core-status`：核心运行状态 / 降级运行（缺失规则集）/ 出站模式；
//! 5. `core-mixed`：经核心 mixed 入站的全链路 HTTPS 请求（DNS 模块 + 路由 + 出站）——
//!    此步失败而其它正常 = 核心侧问题；
//! 6. `outbound-delay`：主 selector 分组经 Clash API 对目标域测速（节点路径正常而
//!    全链路失败 = 入站/路由问题）；
//! 7. `tun-note`：TUN 路径说明（App 自身流量不经本机 VPN，无法从应用内直接探测）。
//!
//! 每一步独立计时、独立成败，互不阻断；报告聚合为步骤数组，前端按状态渲染。

use std::time::{Duration, Instant};

use serde::Serialize;

use crate::config_slices::{ConfigSlices, DnsMode};

/// 诊断步骤状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagStatus {
    Ok,
    Fail,
    /// 前置条件不满足（如核心未运行），附原因。
    Skip,
    /// 纯说明性步骤（无成败）。
    Info,
}

/// 单步诊断结果。
#[derive(Debug, Clone, Serialize)]
pub struct DiagStep {
    /// 稳定标识（`system-dns` / `direct-tcp` / `dns-servers` / `core-status` /
    /// `core-mixed` / `outbound-delay` / `tun-note`）。
    pub key: String,
    /// 展示名（中文）。
    pub name: String,
    pub status: DiagStatus,
    /// 步骤耗时（毫秒；skip/info 为 0）。
    pub duration_ms: u64,
    /// 一行结论。
    pub summary: String,
    /// 详细内容（多行：每服务器/每 IP 一行等）。
    pub detail: String,
}

/// 诊断报告（步骤数组 + 汇总）。
#[derive(Debug, Clone, Serialize)]
pub struct DiagReport {
    pub domain: String,
    pub steps: Vec<DiagStep>,
    /// 失败步骤数（ok/fail 口径）。
    pub failed_steps: usize,
}

/// 诊断输入（命令层收集后传入；本模块不做任何 Tauri / 状态机依赖）。
pub struct DiagnoseInput {
    pub domain: String,
    /// 本地 mixed 入站端口（`ClientConfig::mixed_port`）。
    pub mixed_port: u16,
    /// 核心是否运行中。
    pub core_running: bool,
    /// 降级运行中缺失的内置规则集 tag（空 = 完整分流）。
    pub missing_rule_sets: Vec<String>,
    /// 当前出站模式（rule / global / direct）。
    pub rule_mode: String,
    /// Clash API（启用 + 端口 + 密钥），用于核心内探测。
    pub clash_api: Option<(u16, String)>,
    /// 当前生效的 DNS 服务器集合（命令层已按 takeover / 内置默认展开并过滤弃用）。
    pub dns_servers: Vec<crate::config_slices::DnsServer>,
}

const STEP_TIMEOUT: Duration = Duration::from_secs(8);

fn step(
    key: &str,
    name: &str,
    status: DiagStatus,
    duration: Duration,
    summary: String,
    detail: String,
) -> DiagStep {
    DiagStep {
        key: key.to_string(),
        name: name.to_string(),
        status,
        duration_ms: duration.as_millis() as u64,
        summary,
        detail,
    }
}

/// 汇总当前生效 DNS 服务器集合（takeover → 切片启用项；否则内置默认）。
#[must_use]
pub fn effective_dns_servers(
    slices: &ConfigSlices,
    ipv6_enabled: bool,
) -> Vec<crate::config_slices::DnsServer> {
    if slices.dns.mode == DnsMode::Takeover {
        slices
            .dns
            .servers
            .iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect()
    } else {
        crate::core_config::builtin_dns_slice(ipv6_enabled).servers
    }
}

/// 执行连通性诊断（步骤顺序固定，互不阻断）。
pub async fn diagnose_connectivity(input: DiagnoseInput) -> DiagReport {
    let domain = input.domain.trim().to_string();
    let mut steps = Vec::new();

    steps.push(step_system_dns(&domain).await);
    steps.push(step_direct_tcp(&domain).await);
    steps.push(step_dns_servers(&domain, &input.dns_servers).await);
    steps.push(step_core_status(&input));
    steps.push(step_core_mixed(&domain, &input).await);
    steps.push(step_outbound_delay(&domain, &input).await);
    steps.push(step(
        "tun-note",
        "TUN 路径说明",
        DiagStatus::Info,
        Duration::ZERO,
        "TUN（VPN）路径无法从应用内直接探测".to_string(),
        "Android 上 App 自身流量默认不经本机 VPN（disallowSelf），应用内无法直接测试 tun 入站；\
         若以上各步全部正常但其它 App 仍断网，问题在 tun 参数（stack / auto_route）或系统 VPN 层面。"
            .to_string(),
    ));

    let failed_steps = steps
        .iter()
        .filter(|s| s.status == DiagStatus::Fail)
        .count();
    DiagReport {
        domain,
        steps,
        failed_steps,
    }
}

/// ① 系统 DNS（直连基线）。
async fn step_system_dns(domain: &str) -> DiagStep {
    let started = Instant::now();
    let host = (domain.to_string(), 443);
    match tokio::time::timeout(STEP_TIMEOUT, tokio::net::lookup_host(host)).await {
        Ok(Ok(addrs)) => {
            let ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
            step(
                "system-dns",
                "系统 DNS 解析（直连）",
                if ips.is_empty() { DiagStatus::Fail } else { DiagStatus::Ok },
                started.elapsed(),
                if ips.is_empty() {
                    "系统解析返回空结果".to_string()
                } else {
                    format!("解析到 {} 个地址", ips.len())
                },
                ips.join("\n"),
            )
        }
        Ok(Err(e)) => step(
            "system-dns",
            "系统 DNS 解析（直连）",
            DiagStatus::Fail,
            started.elapsed(),
            format!("系统解析失败: {e}"),
            "设备直连 DNS 不可用（无网络 / DNS 被劫持或拦截）；代理开启后 App 域名解析依赖核心 DNS 模块，此步失败不影响代理路径，仅作基线对照。".to_string(),
        ),
        Err(_) => step(
            "system-dns",
            "系统 DNS 解析（直连）",
            DiagStatus::Fail,
            started.elapsed(),
            "系统解析超时".to_string(),
            String::new(),
        ),
    }
}

/// ② 直连 TCP 443（对照基线；被墙域名预期失败）。
async fn step_direct_tcp(domain: &str) -> DiagStep {
    let started = Instant::now();
    let target = format!("{domain}:443");
    match tokio::time::timeout(STEP_TIMEOUT, tokio::net::TcpStream::connect(target)).await {
        Ok(Ok(_)) => step(
            "direct-tcp",
            "直连 TCP 443",
            DiagStatus::Ok,
            started.elapsed(),
            "直连可达".to_string(),
            String::new(),
        ),
        Ok(Err(e)) => step(
            "direct-tcp",
            "直连 TCP 443",
            DiagStatus::Fail,
            started.elapsed(),
            format!("直连失败: {e}"),
            "境外域名（如 google.com）直连被阻断属预期；境内域名失败则直连网络异常。".to_string(),
        ),
        Err(_) => step(
            "direct-tcp",
            "直连 TCP 443",
            DiagStatus::Fail,
            started.elapsed(),
            "直连超时".to_string(),
            String::new(),
        ),
    }
}

/// ③ 当前生效 DNS 服务器逐项真实查询（延迟 + 应答 IP）。
async fn step_dns_servers(domain: &str, servers: &[crate::config_slices::DnsServer]) -> DiagStep {
    let started = Instant::now();
    if servers.is_empty() {
        return step(
            "dns-servers",
            "DNS 服务器查询",
            DiagStatus::Fail,
            Duration::ZERO,
            "没有启用的 DNS 服务器".to_string(),
            "全部服务器被弃用时核心无可用解析器。".to_string(),
        );
    }
    let mut lines = Vec::new();
    let mut failures = 0usize;
    for server in servers {
        let label = if server.name.trim().is_empty() {
            server.tag.clone()
        } else {
            format!("{}（{}）", server.name.trim(), server.tag)
        };
        match crate::dns_probe::probe_dns_server_full(&crate::dns_probe::DnsProbeInput {
            server_type: server.server_type.as_str().to_string(),
            server: server.server.clone(),
            server_port: server.server_port,
            domain: Some(domain.to_string()),
        })
        .await
        {
            Ok(result) => {
                let answers = if result.answers.is_empty() {
                    "无 A/AAAA 应答".to_string()
                } else {
                    result.answers.join(" ")
                };
                lines.push(format!("✓ {label}: {}ms · {answers}", result.latency_ms));
            }
            Err(e) => {
                failures += 1;
                lines.push(format!("✗ {label}: {e}"));
            }
        }
    }
    step(
        "dns-servers",
        "DNS 服务器查询",
        if failures == 0 {
            DiagStatus::Ok
        } else {
            DiagStatus::Fail
        },
        started.elapsed(),
        format!("{} 个服务器，{} 个失败", servers.len(), failures),
        lines.join("\n"),
    )
}

/// ④ 核心运行状态（含降级运行提示）。
fn step_core_status(input: &DiagnoseInput) -> DiagStep {
    if !input.core_running {
        return step(
            "core-status",
            "核心运行状态",
            DiagStatus::Fail,
            Duration::ZERO,
            "核心未运行".to_string(),
            "核心未运行时 DNS 劫持 / 路由分流均不生效；先启动代理再诊断。".to_string(),
        );
    }
    if !input.missing_rule_sets.is_empty() {
        return step(
            "core-status",
            "核心运行状态",
            DiagStatus::Fail,
            Duration::ZERO,
            format!(
                "降级运行：{} 个内置规则集未就绪",
                input.missing_rule_sets.len()
            ),
            format!(
                "缺失：{}。CN 分流退化为 route.final 兜底（几乎全部流量走代理）；后台重试补齐后会自动恢复。",
                input.missing_rule_sets.join("、")
            ),
        );
    }
    step(
        "core-status",
        "核心运行状态",
        DiagStatus::Ok,
        Duration::ZERO,
        format!("核心运行中（出站模式 {}）", input.rule_mode),
        String::new(),
    )
}

/// ⑤ 经核心 mixed 入站的全链路 HTTPS（核心 DNS 模块 + 路由 + 出站）。
async fn step_core_mixed(domain: &str, input: &DiagnoseInput) -> DiagStep {
    if !input.core_running {
        return step(
            "core-mixed",
            "核心全链路（mixed 入站）",
            DiagStatus::Skip,
            Duration::ZERO,
            "核心未运行，跳过".to_string(),
            String::new(),
        );
    }
    let started = Instant::now();
    let proxy = match reqwest::Proxy::all(format!("http://127.0.0.1:{}", input.mixed_port)) {
        Ok(p) => p,
        Err(e) => {
            return step(
                "core-mixed",
                "核心全链路（mixed 入站）",
                DiagStatus::Fail,
                started.elapsed(),
                format!("代理 URL 非法: {e}"),
                String::new(),
            );
        }
    };
    let client = reqwest::Client::builder()
        .proxy(proxy)
        .timeout(STEP_TIMEOUT)
        .redirect(reqwest::redirect::Policy::none())
        .build();
    let Ok(client) = client else {
        return step(
            "core-mixed",
            "核心全链路（mixed 入站）",
            DiagStatus::Fail,
            started.elapsed(),
            "HTTP 客户端构建失败".to_string(),
            String::new(),
        );
    };
    let url = format!("https://{domain}/generate_204");
    match client.get(&url).send().await {
        Ok(resp) => step(
            "core-mixed",
            "核心全链路（mixed 入站）",
            DiagStatus::Ok,
            started.elapsed(),
            format!("经核心访问成功（HTTP {}）", resp.status()),
            "该请求路径 = 应用流量在核心内的完整路径（DNS 模块解析 → 路由分流 → 出站）。".to_string(),
        ),
        Err(e) => step(
            "core-mixed",
            "核心全链路（mixed 入站）",
            DiagStatus::Fail,
            started.elapsed(),
            format!("经核心访问失败: {e}"),
            "核心 DNS / 路由 / 出站链路之一失败。对照上方各 DNS 服务器查询结果定位：全部 DNS 失败则核心\
             DNS 模块异常（规则集缺失 / DNS 配置错误）；DNS 正常而本步失败则出站或路由问题。"
                .to_string(),
        ),
    }
}

/// ⑥ 主 selector 分组经 Clash API 对目标域测速（节点路径对照）。
async fn step_outbound_delay(domain: &str, input: &DiagnoseInput) -> DiagStep {
    let name = "outbound-delay";
    let label = "代理出站延迟（目标域）";
    if !input.core_running {
        return step(
            name,
            label,
            DiagStatus::Skip,
            Duration::ZERO,
            "核心未运行，跳过".to_string(),
            String::new(),
        );
    }
    let Some((port, secret)) = &input.clash_api else {
        return step(
            name,
            label,
            DiagStatus::Skip,
            Duration::ZERO,
            "Clash API 未启用，跳过".to_string(),
            "在 配置 → 实验性配置 中确认 Clash API 端口与密钥后可启用本步骤。".to_string(),
        );
    };
    let started = Instant::now();
    let proxies = match crate::proxies::clash_get_proxies(*port, secret).await {
        Ok(p) => p,
        Err(e) => {
            return step(
                name,
                label,
                DiagStatus::Fail,
                started.elapsed(),
                format!("读取分组失败: {e}"),
                String::new(),
            );
        }
    };
    // 主 selector 分组：优先 tag 为 proxy 的分组，否则第一个 Selector。
    let group = proxies
        .groups
        .iter()
        .find(|g| g.name == "proxy")
        .or_else(|| proxies.groups.iter().find(|g| g.group_type == "Selector"));
    let Some(group) = group else {
        return step(
            name,
            label,
            DiagStatus::Skip,
            started.elapsed(),
            "没有可用的代理分组".to_string(),
            String::new(),
        );
    };
    let test_url = format!("https://{domain}/generate_204");
    match crate::proxies::clash_test_delay(*port, secret, &group.name, Some(&test_url), 5000).await
    {
        Ok(Some(ms)) => step(
            name,
            label,
            DiagStatus::Ok,
            started.elapsed(),
            format!("分组「{}」当前节点 {}：{}ms", group.name, group.now, ms),
            "节点路径正常；若核心全链路失败而本步正常，问题在核心 DNS / 路由 / 入站侧。"
                .to_string(),
        ),
        Ok(None) => step(
            name,
            label,
            DiagStatus::Fail,
            started.elapsed(),
            format!("分组「{}」当前节点 {} 测速失败", group.name, group.now),
            "当前节点对目标域不可达；尝试在面板中切换节点或重试。".to_string(),
        ),
        Err(e) => step(
            name,
            label,
            DiagStatus::Fail,
            started.elapsed(),
            format!("测速请求失败: {e}"),
            String::new(),
        ),
    }
}

#[cfg(test)]
mod tests;
