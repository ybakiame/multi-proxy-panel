//! Tauri 命令层：把 `pp-client` 内部类型包装为 serde 视图结构，供前端调用。
//!
//! 视图类型（`*View`）是独立的 serde 简单结构，避免把内部类型直接暴露给
//! 前端；内部类型与视图的转换通过字段映射与 `serde_json` 往返完成。

use std::path::PathBuf;

use pp_client::{ClientConfig, ClientState, RemoteManager, RemoteResource};
use pp_common::CoreType;
use pp_mitm::TrafficRecorder;
use pp_script::{ScriptDialect, TaskScriptView};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// 客户端配置的对外视图（serde 简单结构，避免直接暴露内部类型）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfigView {
    /// 数据目录（展示用；持久化路径由应用状态决定）。
    pub data_dir: String,
    pub hub_url: String,
    pub sub_token: String,
    /// 核心类型：`singbox` / `mihomo`（与 `pp_common::CoreType` 的 serde 表示一致）。
    pub core_type: String,
    pub core_binary: String,
    pub mixed_port: u16,
    pub mitm_enabled: bool,
    pub mitm_hostnames: Vec<String>,
    /// MITM 脚本方言：`Surge` / `QuantumultX` / `Loon`。
    pub mitm_script_dialect: String,
    pub system_proxy_enabled: bool,
}

impl ClientConfigView {
    /// 从内部配置构造视图。
    fn from_config(cfg: &ClientConfig) -> Self {
        Self {
            data_dir: cfg.data_dir.to_string_lossy().into_owned(),
            hub_url: cfg.hub_url.clone(),
            sub_token: cfg.sub_token.clone(),
            core_type: serde_json::to_value(cfg.core_type)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            core_binary: cfg.core_binary.to_string_lossy().into_owned(),
            mixed_port: cfg.mixed_port,
            mitm_enabled: cfg.mitm_enabled,
            mitm_hostnames: cfg.mitm.hostnames.clone(),
            mitm_script_dialect: serde_json::to_value(cfg.mitm.script_dialect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            system_proxy_enabled: cfg.system_proxy_enabled,
        }
    }

    /// 转为内部配置；`data_dir` 由应用状态决定（视图中的 `data_dir` 仅作展示）。
    fn into_config(self, data_dir: &std::path::Path) -> Result<ClientConfig, String> {
        let value = serde_json::json!({
            "data_dir": data_dir,
            "hub_url": self.hub_url,
            "sub_token": self.sub_token,
            "core_type": self.core_type,
            "core_binary": self.core_binary,
            "mixed_port": self.mixed_port,
            "mitm_enabled": self.mitm_enabled,
            "mitm": {
                "ca_dir": data_dir.join("certs"),
                "hostnames": self.mitm_hostnames,
                "script_dialect": self.mitm_script_dialect,
            },
            "system_proxy_enabled": self.system_proxy_enabled,
        });
        serde_json::from_value::<ClientConfig>(value).map_err(|e| e.to_string())
    }
}

/// 客户端运行状态的对外视图。
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatusView {
    pub core_running: bool,
    pub mitm_addr: Option<String>,
    pub system_proxy: bool,
}

impl ClientStatusView {
    fn from_status(status: &pp_client::ClientStatus) -> Self {
        Self {
            core_running: status.core_running,
            mitm_addr: status.mitm_addr.map(|a| a.to_string()),
            system_proxy: status.system_proxy,
        }
    }
}

/// 一条流量记录的对外视图。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrafficRecordView {
    pub id: String,
    pub method: String,
    pub url: String,
    pub request_headers: Vec<(String, String)>,
    pub request_body: Option<String>,
    pub response_status: u16,
    pub response_headers: Vec<(String, String)>,
    pub response_body: Option<String>,
    pub timestamp: String,
    pub duration_ms: u64,
}

impl TrafficRecordView {
    /// 从内部流量记录构造视图。
    fn from_record(rec: &pp_mitm::TrafficRecord) -> Self {
        Self {
            id: rec.id.to_string(),
            method: rec.method.clone(),
            url: rec.url.clone(),
            request_headers: rec.request_headers.clone(),
            request_body: rec.request_body.clone(),
            response_status: rec.response_status,
            response_headers: rec.response_headers.clone(),
            response_body: rec.response_body.clone(),
            timestamp: rec.timestamp.to_rfc3339(),
            duration_ms: rec.duration_ms,
        }
    }
}

/// 读取当前配置（`data_dir/client.json` 不存在时返回默认值）。
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<ClientConfigView, String> {
    let config_path = state.data_dir.join("client.json");
    let cfg = if config_path.exists() {
        ClientConfig::load(&state.data_dir).map_err(|e| format!("读取配置失败: {e}"))?
    } else {
        ClientConfig::new(
            state.data_dir.clone(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        )
    };
    Ok(ClientConfigView::from_config(&cfg))
}

/// 保存配置（校验 hub_url / sub_token 非空）。
#[tauri::command]
pub async fn save_config(state: State<'_, AppState>, cfg: ClientConfigView) -> Result<(), String> {
    if cfg.hub_url.trim().is_empty() {
        return Err("hub_url 不能为空".to_string());
    }
    if cfg.sub_token.trim().is_empty() {
        return Err("sub_token 不能为空".to_string());
    }
    let config = cfg.into_config(&state.data_dir)?;
    config.save().map_err(|e| format!("保存配置失败: {e}"))
}

/// 启动代理（无运行状态时先基于已保存配置新建）。
#[tauri::command]
pub async fn start_proxy(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    if lock.is_none() {
        let cfg = ClientConfig::load(&state.data_dir)
            .map_err(|e| format!("未找到已保存的配置（{e}），请先保存配置"))?;
        *lock = Some(ClientState::new(cfg));
    }
    let client = lock.as_mut().ok_or("客户端状态初始化失败")?;
    client.start().await.map_err(|e| format!("启动失败: {e}"))?;
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// 停止代理。
#[tauri::command]
pub async fn stop_proxy(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    let Some(client) = lock.as_mut() else {
        return Ok(ClientStatusView::default());
    };
    client.stop().await;
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// 查询代理运行状态。
#[tauri::command]
pub async fn proxy_status(state: State<'_, AppState>) -> Result<ClientStatusView, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(ClientStatusView::default());
    };
    let status = client.status().await;
    Ok(ClientStatusView::from_status(&status))
}

/// 查询 MITM 流量记录。
///
/// 有运行中的 `ClientState` 时通过 `ClientState::recorder()` 取回
/// `MemoryRecorder` 并按时间序映射为视图；未启动时返回空列表。
#[tauri::command]
pub async fn list_traffic(state: State<'_, AppState>) -> Result<Vec<TrafficRecordView>, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(Vec::new());
    };
    let records = client.recorder().list();
    Ok(records
        .iter()
        .map(TrafficRecordView::from_record)
        .collect())
}

/// 一条远程订阅资源的对外视图。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteResourceView {
    pub name: String,
    pub url: String,
    /// 资源类型：`Script` / `Snippet`。
    pub kind: String,
    /// 脚本方言：`Surge` / `QuantumultX` / `Loon`。
    pub dialect: String,
    pub update_interval_secs: u64,
    pub enabled: bool,
}

impl RemoteResourceView {
    fn from_remote(remote: &RemoteResource) -> Self {
        Self {
            name: remote.name.clone(),
            url: remote.url.clone(),
            kind: serde_json::to_value(remote.kind)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            dialect: serde_json::to_value(remote.dialect)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            update_interval_secs: remote.update_interval_secs,
            enabled: remote.enabled,
        }
    }

    fn into_remote(self) -> Result<RemoteResource, String> {
        let value = serde_json::json!({
            "name": self.name,
            "url": self.url,
            "kind": self.kind,
            "dialect": self.dialect,
            "update_interval_secs": self.update_interval_secs,
            "enabled": self.enabled,
        });
        serde_json::from_value::<RemoteResource>(value).map_err(|e| e.to_string())
    }
}

/// 一次 `fetch_remotes` 拉取报告的对外视图。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchReportView {
    pub fetched: usize,
    pub scripts: usize,
    pub rewrites: usize,
    pub tasks: usize,
    pub warnings: Vec<String>,
}

/// 一次配置导入摘要的对外视图。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummaryView {
    pub rewrites: usize,
    pub scripts: usize,
    pub tasks: usize,
    pub hostnames: usize,
    pub warnings: Vec<String>,
}

/// 列出全部远程资源（`remotes.json` 清单）。
#[tauri::command]
pub async fn list_remotes(state: State<'_, AppState>) -> Result<Vec<RemoteResourceView>, String> {
    let manager = RemoteManager::new(state.data_dir.clone());
    let remotes = manager.load().map_err(|e| format!("读取远程资源失败: {e}"))?;
    Ok(remotes.iter().map(RemoteResourceView::from_remote).collect())
}

/// 新增一条远程资源；重名时报错。
#[tauri::command]
pub async fn add_remote(
    state: State<'_, AppState>,
    remote: RemoteResourceView,
) -> Result<(), String> {
    if remote.name.trim().is_empty() {
        return Err("资源名不能为空".to_string());
    }
    if remote.url.trim().is_empty() {
        return Err("URL 不能为空".to_string());
    }
    let remote = remote.into_remote()?;
    let manager = RemoteManager::new(state.data_dir.clone());
    let mut remotes = manager.load().map_err(|e| format!("读取远程资源失败: {e}"))?;
    if remotes.iter().any(|r| r.name == remote.name) {
        return Err(format!("远程资源 '{}' 已存在", remote.name));
    }
    remotes.push(remote);
    manager
        .save(&remotes)
        .map_err(|e| format!("保存远程资源失败: {e}"))
}

/// 删除一条远程资源；不存在时报错。
#[tauri::command]
pub async fn remove_remote(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let manager = RemoteManager::new(state.data_dir.clone());
    let mut remotes = manager.load().map_err(|e| format!("读取远程资源失败: {e}"))?;
    let before = remotes.len();
    remotes.retain(|r| r.name != name);
    if remotes.len() == before {
        return Err(format!("远程资源 '{}' 不存在", name));
    }
    manager
        .save(&remotes)
        .map_err(|e| format!("保存远程资源失败: {e}"))
}

/// 拉取全部启用的远程资源（无系统代理，30 秒超时）。
#[tauri::command]
pub async fn fetch_remotes(state: State<'_, AppState>) -> Result<FetchReportView, String> {
    let manager = RemoteManager::new(state.data_dir.clone());
    let remotes = manager.load().map_err(|e| format!("读取远程资源失败: {e}"))?;
    let report = manager.fetch_all(&remotes).await;
    Ok(FetchReportView {
        fetched: report.fetched,
        scripts: report.scripts,
        rewrites: report.rewrites,
        tasks: report.tasks,
        warnings: report.warnings,
    })
}

/// 列出定时任务；客户端未启动或调度器未就绪时返回空列表。
#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskScriptView>, String> {
    let lock = state.client.lock().await;
    let Some(client) = lock.as_ref() else {
        return Ok(Vec::new());
    };
    let Some(scheduler) = client.scheduler() else {
        return Ok(Vec::new());
    };
    Ok(scheduler.list_tasks())
}

/// 手动运行一个定时任务；返回脚本 `$done` 输出的 JSON 字符串。
///
/// QuickJS 运行时非 Send，脚本执行放到阻塞线程上的独立 current_thread runtime 驱动。
#[tauri::command]
pub async fn run_task(state: State<'_, AppState>, name: String) -> Result<String, String> {
    let scheduler = {
        let lock = state.client.lock().await;
        let client = lock
            .as_ref()
            .ok_or_else(|| "客户端未启动，无法运行任务".to_string())?;
        client
            .scheduler_handle()
            .ok_or_else(|| "任务调度器未就绪".to_string())?
    };
    let output = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("构建脚本运行时失败: {e}"))?;
        let result = rt
            .block_on(scheduler.run_now(&name))
            .map_err(|e| format!("运行任务失败: {e}"))?;
        Ok(result.0.to_string())
    })
    .await
    .map_err(|e| format!("任务线程异常: {e}"))??;
    Ok(output)
}

/// 导入 QX / Surge / Loon 配置片段：解析后合并写入本地导入缓存
/// `remote_cache/imported.json`（脚本 / 任务 URL 不拉取，source 为空则跳过计 warning）。
#[tauri::command]
pub async fn import_config(
    state: State<'_, AppState>,
    content: String,
    dialect: String,
) -> Result<ImportSummaryView, String> {
    let script_dialect = match dialect.as_str() {
        "quantumultx" => ScriptDialect::QuantumultX,
        "surge" => ScriptDialect::Surge,
        "loon" => ScriptDialect::Loon,
        other => {
            return Err(format!(
                "未知方言 '{other}'（可选: quantumultx / surge / loon）"
            ));
        }
    };
    let imported = pp_client::parse_import(&content, script_dialect)
        .map_err(|e| format!("解析配置失败: {e}"))?;
    let manager = RemoteManager::new(state.data_dir.clone());
    let summary = manager
        .merge_imported(&imported)
        .map_err(|e| format!("写入导入缓存失败: {e}"))?;
    Ok(ImportSummaryView {
        rewrites: summary.rewrites,
        scripts: summary.scripts,
        tasks: summary.tasks,
        hostnames: summary.hostnames,
        warnings: summary.warnings,
    })
}
