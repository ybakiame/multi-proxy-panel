//! 连通性诊断命令（开发者工具）。
//!
//! 前端开发者工具页对单个域名发起一次分步诊断；本命令负责收集运行上下文
//! （核心状态 / Clash API / mixed 端口 / 当前生效 DNS 服务器）后委托
//! [`pp_client::diagnose::diagnose_connectivity`]（纯逻辑，可脱离 Tauri 单测）。

use pp_client::ClientConfig;
use pp_client::config_slices::ConfigSlicesStore;
use pp_client::diagnose::{
    DiagReport, DiagnoseInput, diagnose_connectivity, effective_dns_servers,
};
use serde::Deserialize;
use tauri::State;

use crate::state::AppState;

/// 默认诊断目标域。
const DEFAULT_DOMAIN: &str = "google.com";

#[derive(Debug, Clone, Deserialize)]
pub struct DiagnoseInputArgs {
    /// 目标域名（缺省 `google.com`）。
    pub domain: Option<String>,
}

// ---------------------------------------------------------------------------
// Pure command bodies (unit-testable without a Tauri runtime)
// ---------------------------------------------------------------------------

/// 组装诊断输入（从 data_dir 读配置与切片；核心状态由调用方从状态机读取后传入）。
pub(crate) fn build_diagnose_input(
    data_dir: &std::path::Path,
    domain: Option<String>,
    core_running: bool,
    missing_rule_sets: Vec<String>,
) -> Result<DiagnoseInput, String> {
    // client.json 缺失/损坏时回退默认配置（诊断工具不因配置缺失而不可用）。
    let cfg = ClientConfig::load(data_dir).unwrap_or_else(|_| {
        ClientConfig::new(
            data_dir.to_path_buf(),
            String::new(),
            String::new(),
            std::path::PathBuf::new(),
        )
    });
    let slices = ConfigSlicesStore::new(data_dir.to_path_buf())
        .load()
        .map_err(|e| format!("读取配置切片失败: {e}"))?;
    let domain = domain
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .unwrap_or(DEFAULT_DOMAIN)
        .to_string();
    Ok(DiagnoseInput {
        domain,
        mixed_port: cfg.mixed_port,
        core_running,
        missing_rule_sets,
        rule_mode: cfg.normalized_rule_mode().to_string(),
        clash_api: cfg
            .clash_api_enabled
            .then_some((cfg.clash_api_port, cfg.clash_api_secret.clone())),
        dns_servers: effective_dns_servers(&slices, cfg.ipv6_enabled),
    })
}

// ---------------------------------------------------------------------------
// Tauri commands
// ---------------------------------------------------------------------------

/// 连通性诊断：对目标域名沿流量路径分层检查（系统 DNS / 直连 / 各 DNS 服务器 /
/// 核心状态 / 经核心全链路 / 出站延迟）。
#[tauri::command]
pub async fn diagnose_connectivity_run(
    state: State<'_, AppState>,
    input: DiagnoseInputArgs,
) -> Result<DiagReport, String> {
    let (core_running, missing_rule_sets) = {
        let lock = state.client.lock().await;
        match lock.as_ref() {
            Some(client) => {
                let status = client.status().await;
                (status.core_running, status.missing_rule_sets)
            }
            None => (false, Vec::new()),
        }
    };
    let diag_input = build_diagnose_input(
        &state.data_dir,
        input.domain,
        core_running,
        missing_rule_sets,
    )?;
    Ok(diagnose_connectivity(diag_input).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// build_diagnose_input：域名缺省回退 google.com；配置缺失时不崩溃（ClientConfig::load
    /// 缺文件返回默认配置，与保存路径语义一致）。
    #[test]
    fn build_input_defaults_and_builtin_dns() {
        let dir = tempfile::tempdir().unwrap();
        let input = build_diagnose_input(dir.path(), None, false, Vec::new()).unwrap();
        assert_eq!(input.domain, "google.com");
        assert_eq!(
            input.dns_servers.len(),
            2,
            "follow_system uses builtin local/remote"
        );
        assert!(!input.core_running);

        let input =
            build_diagnose_input(dir.path(), Some("  example.com ".to_string()), true, vec![])
                .unwrap();
        assert_eq!(input.domain, "example.com");
        assert!(input.core_running);
    }
}
