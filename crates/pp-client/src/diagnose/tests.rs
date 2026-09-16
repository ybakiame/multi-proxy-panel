//! `diagnose` 模块单元测试。

use super::*;
use crate::config_slices::{ConfigSlices, DnsServer, DnsServerType};

fn base_input(core_running: bool) -> DiagnoseInput {
    DiagnoseInput {
        domain: "google.com".to_string(),
        mixed_port: 17890,
        core_running,
        missing_rule_sets: Vec::new(),
        rule_mode: "rule".to_string(),
        clash_api: None,
        dns_servers: Vec::new(),
    }
}

/// 核心未运行：mixed 全链路与出站延迟两步必须 skip（而非 fail）。
#[tokio::test]
async fn core_dependent_steps_skip_when_not_running() {
    let report = diagnose_connectivity(base_input(false)).await;
    let mixed = report.steps.iter().find(|s| s.key == "core-mixed").unwrap();
    let delay = report
        .steps
        .iter()
        .find(|s| s.key == "outbound-delay")
        .unwrap();
    assert_eq!(mixed.status, DiagStatus::Skip);
    assert_eq!(delay.status, DiagStatus::Skip);
    assert_eq!(
        report
            .steps
            .iter()
            .find(|s| s.key == "core-status")
            .unwrap()
            .status,
        DiagStatus::Fail,
        "core not running must fail the core-status step"
    );
}

/// 降级运行：missing_rule_sets 非空时 core-status 报 fail 并列出缺失 tag。
#[tokio::test]
async fn degraded_core_status_fails_with_missing_sets() {
    let mut input = base_input(true);
    input.missing_rule_sets = vec!["geoip-cn".to_string()];
    let report = diagnose_connectivity(input).await;
    let step = report
        .steps
        .iter()
        .find(|s| s.key == "core-status")
        .unwrap();
    assert_eq!(step.status, DiagStatus::Fail);
    assert!(step.detail.contains("geoip-cn"));
}

/// 无启用 DNS 服务器：dns-servers 步骤 fail；有服务器时按类型尝试
/// （udp 不可达域名服务器在测试环境可能失败，但不允许 panic）。
#[tokio::test]
async fn dns_servers_step_handles_empty_and_unreachable() {
    let input = base_input(false);
    let report = diagnose_connectivity(input.clone_inner()).await;
    let step = report
        .steps
        .iter()
        .find(|s| s.key == "dns-servers")
        .unwrap();
    assert_eq!(step.status, DiagStatus::Fail);
    assert!(step.summary.contains("没有启用"));

    let mut with_server = base_input(false);
    with_server.dns_servers = vec![DnsServer {
        tag: "dead".to_string(),
        server: "127.0.0.1".to_string(),
        server_type: DnsServerType::Udp,
        server_port: Some(1),
        ..Default::default()
    }];
    let report = diagnose_connectivity(with_server).await;
    let step = report
        .steps
        .iter()
        .find(|s| s.key == "dns-servers")
        .unwrap();
    assert_eq!(
        step.status,
        DiagStatus::Fail,
        "unreachable server must fail"
    );
    assert!(step.detail.contains('✗'));
}

/// effective_dns_servers：takeover 用切片启用项，否则内置默认。
#[test]
fn effective_servers_takeover_vs_builtin() {
    let mut slices = ConfigSlices::default();
    let builtin = effective_dns_servers(&slices, false);
    assert_eq!(
        builtin.len(),
        2,
        "follow_system falls back to builtin local/remote"
    );

    slices.dns.mode = DnsMode::Takeover;
    slices.dns.servers = vec![
        DnsServer {
            tag: "a".to_string(),
            server: "1.1.1.1".to_string(),
            server_type: DnsServerType::Udp,
            ..Default::default()
        },
        DnsServer {
            tag: "b-disabled".to_string(),
            server: "8.8.8.8".to_string(),
            server_type: DnsServerType::Udp,
            enabled: false,
            ..Default::default()
        },
    ];
    let takeover = effective_dns_servers(&slices, false);
    assert_eq!(takeover.len(), 1, "disabled server excluded");
    assert_eq!(takeover[0].tag, "a");
}

impl DiagnoseInput {
    fn clone_inner(&self) -> Self {
        DiagnoseInput {
            domain: self.domain.clone(),
            mixed_port: self.mixed_port,
            core_running: self.core_running,
            missing_rule_sets: self.missing_rule_sets.clone(),
            rule_mode: self.rule_mode.clone(),
            clash_api: self.clash_api.clone(),
            dns_servers: self.dns_servers.clone(),
        }
    }
}
