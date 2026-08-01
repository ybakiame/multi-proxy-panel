//! Tauri 命令层：把 `pp-client` 内部类型包装为 serde 视图结构，供前端调用。
//!
//! 视图类型（`*View`）是独立的 serde 简单结构，避免把内部类型直接暴露给
//! 前端；内部类型与视图的转换通过字段映射与 `serde_json` 往返完成。

use std::path::PathBuf;
use std::sync::Arc;

use pp_client::{
    build_core_config, compose_mihomo_config, compose_singbox_config, ClientConfig, ClientState,
    ProfileOverrides, ProfileStore, RemoteManager, RemoteResource, SubContent, SubscriptionFetcher,
};
use pp_common::CoreType;
use pp_mitm::TrafficRecorder;
use pp_script::{ScriptDialect, TaskScriptView};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

/// 通过 tauri-plugin-notification 发送 OS 桌面通知的 [`pp_script::Notifier`]。
///
/// 通知发送失败时回退为 `tracing::warn` 日志，不阻断脚本执行。
pub struct TauriNotifier {
    app: tauri::AppHandle,
}

impl TauriNotifier {
    /// 使用应用句柄创建通知器。
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl pp_script::Notifier for TauriNotifier {
    fn notify(&self, title: &str, subtitle: &str, body: &str, _options: Option<serde_json::Value>) {
        use tauri_plugin_notification::NotificationExt;
        if let Err(e) = self
            .app
            .notification()
            .builder()
            .title(title)
            .body(format!("{subtitle}\n{body}"))
            .show()
        {
            tracing::warn!(error = %e, "发送桌面通知失败");
        }
    }
}

/// 客户端配置的对外视图（serde 简单结构，避免直接暴露内部类型）。
///
/// 注意：所有 `*View` 结构体按字段名原样（snake_case）序列化，与前端
/// `src/api.ts` 的 TS 类型逐字段对齐；曾一度使用 `rename_all = "camelCase"`
/// 导致前端读取 `mitm_hostnames` 等字段全部为 `undefined` 而崩溃。
#[derive(Debug, Clone, Serialize, Deserialize)]
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
///
/// `ClientState` 注入 [`TauriNotifier`]：脚本 `$notify` / `$notification` 通过
/// tauri-plugin-notification 发送 OS 桌面通知。
///
/// 状态机的启动流程经 `run_blocking` 在独立线程驱动：`ClientState::start`
/// 内部 `build_core_config` 的 future 含 rquickjs 非 `Send` 结构，不能直接在
/// Tauri 命令（要求 `Send` future）中 `await`。
#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ClientStatusView, String> {
    let client = Arc::clone(&state.client);
    let data_dir = state.data_dir.clone();
    let result = run_blocking(move || async move {
        let mut lock = client.lock().await;
        if lock.is_none() {
            let cfg = ClientConfig::load(&data_dir)
                .map_err(|e| format!("未找到已保存的配置（{e}），请先保存配置"))?;
            *lock = Some(ClientState::with_notifier(
                cfg,
                Arc::new(TauriNotifier::new(app)),
            ));
        }
        let client = lock
            .as_mut()
            .ok_or_else(|| "客户端状态初始化失败".to_string())?;
        client.start().await.map_err(|e| format!("启动失败: {e}"))?;
        let status = client.status().await;
        Ok(ClientStatusView::from_status(&status))
    })
    .await?;
    Ok(result)
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
    Ok(records.iter().map(TrafficRecordView::from_record).collect())
}

/// 一条远程订阅资源的对外视图。
#[derive(Debug, Clone, Serialize, Deserialize)]
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
pub struct FetchReportView {
    pub fetched: usize,
    pub scripts: usize,
    pub rewrites: usize,
    pub tasks: usize,
    pub warnings: Vec<String>,
}

/// 一次配置导入摘要的对外视图。
#[derive(Debug, Clone, Serialize)]
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
    let remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
    Ok(remotes
        .iter()
        .map(RemoteResourceView::from_remote)
        .collect())
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
    let mut remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
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
    let mut remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
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
    let remotes = manager
        .load()
        .map_err(|e| format!("读取远程资源失败: {e}"))?;
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
/// 脚本执行由 pp-script 的 `ScriptWorker` 在专有线程驱动（`Send` future）；
/// 但 `scheduler.run_now` 的 future 含非 `Send` 结构，经 `run_blocking` 在
/// 独立线程上驱动。
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
    let result = run_blocking(move || async move {
        let output = scheduler
            .run_now(&name)
            .await
            .map_err(|e| format!("运行任务失败: {e}"))?;
        Ok(output.0.to_string())
    })
    .await?;
    Ok(result)
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

/// Profile 复写配置的对外视图（与前端 `ProfileOverrides` TS 类型逐字段对齐）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileOverridesView {
    /// YAML 深合并复写（空串 = 未启用）。
    pub yaml_override: String,
    /// JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。
    pub js_override: String,
}

impl ProfileOverridesView {
    fn from_overrides(ov: &ProfileOverrides) -> Self {
        Self {
            yaml_override: ov.yaml_override.clone(),
            js_override: ov.js_override.clone(),
        }
    }
}

/// 读取 Profile 复写配置；`data_dir/profile.json` 不存在或损坏时返回空串默认。
#[tauri::command]
pub fn get_profile_overrides(state: State<'_, AppState>) -> Result<ProfileOverridesView, String> {
    let store = ProfileStore::new(state.data_dir.clone());
    let overrides = store.load().map_err(|e| format!("读取复写配置失败: {e}"))?;
    Ok(ProfileOverridesView::from_overrides(&overrides))
}

/// 校验 YAML 复写：非空时必须是可解析的 YAML mapping（与 `apply_yaml_override` 一致）。
fn validate_yaml_override(yaml: &str) -> Result<(), String> {
    if yaml.trim().is_empty() {
        return Ok(());
    }
    let patch: serde_json::Value =
        serde_yaml::from_str(yaml).map_err(|e| format!("YAML 复写解析失败: {e}"))?;
    if patch.is_null() {
        return Ok(());
    }
    if !patch.is_object() {
        return Err("YAML 复写必须是 mapping（对象）".to_string());
    }
    Ok(())
}

/// 粗检 JS 复写是否定义了 `main` 函数（同步纯函数模式）。
fn validate_js_override(js: &str) -> Result<(), String> {
    if js.trim().is_empty() {
        return Ok(());
    }
    if !js.contains("function main") && !js.contains("main(") {
        return Err(
            "JS 复写需定义 main 函数（function main(config) { ... return config; }）".to_string(),
        );
    }
    Ok(())
}

/// 保存 Profile 复写配置到 `data_dir/profile.json`（校验失败不落盘）。
#[tauri::command]
pub fn save_profile_overrides(
    state: State<'_, AppState>,
    ov: ProfileOverridesView,
) -> Result<(), String> {
    validate_yaml_override(&ov.yaml_override)?;
    validate_js_override(&ov.js_override)?;
    let store = ProfileStore::new(state.data_dir.clone());
    let overrides = ProfileOverrides {
        yaml_override: ov.yaml_override,
        js_override: ov.js_override,
    };
    store
        .save(&overrides)
        .map_err(|e| format!("保存复写配置失败: {e}"))
}

/// 生成生效配置预览：拉取订阅 → 内置模板 → YAML/JS 复写 → 核心合成（不含 MITM 链路）。
///
/// 返回最终核心可用的配置文本（sing-box 为 JSON、mihomo 为 YAML），供只读预览。
/// 需要已保存的客户端配置（`data_dir/client.json`，含 hub_url / sub_token / core_type）。
#[tauri::command]
pub async fn preview_core_config(state: State<'_, AppState>) -> Result<String, String> {
    let data_dir = state.data_dir.clone();
    run_blocking(move || preview_config_async(data_dir)).await
}

/// 预览的具体实现；future 含非 `Send` 结构，由 `run_blocking` 在独立线程驱动。
async fn preview_config_async(data_dir: std::path::PathBuf) -> Result<String, String> {
    let cfg = ClientConfig::load(&data_dir)
        .map_err(|e| format!("未找到已保存的配置（{e}），请先在设置页保存配置"))?;
    let store = ProfileStore::new(data_dir);
    let overrides = store.load().map_err(|e| format!("读取复写配置失败: {e}"))?;

    let fetcher = SubscriptionFetcher::new();
    let sub_content = match cfg.core_type {
        CoreType::SingBox => {
            let (config, _) = fetcher
                .fetch_singbox_config(&cfg.hub_url, &cfg.sub_token)
                .await
                .map_err(|e| format!("拉取订阅失败: {e}"))?;
            SubContent::SingBox(config)
        }
        CoreType::Mihomo => {
            let (yaml, _) = fetcher
                .fetch_clash_config(&cfg.hub_url, &cfg.sub_token)
                .await
                .map_err(|e| format!("拉取订阅失败: {e}"))?;
            SubContent::Mihomo(yaml)
        }
    };

    let profile_cfg = build_core_config(cfg.core_type, &sub_content, &overrides)
        .await
        .map_err(|e| format!("生成配置失败: {e}"))?;

    match cfg.core_type {
        CoreType::SingBox => {
            let value = compose_singbox_config(&profile_cfg, cfg.mixed_port, None)
                .map_err(|e| format!("合成 sing-box 配置失败: {e}"))?;
            serde_json::to_string_pretty(&value).map_err(|e| format!("序列化配置失败: {e}"))
        }
        CoreType::Mihomo => {
            let yaml =
                serde_yaml::to_string(&profile_cfg).map_err(|e| format!("序列化配置失败: {e}"))?;
            let value = compose_mihomo_config(&yaml, cfg.mixed_port, None)
                .map_err(|e| format!("合成 mihomo 配置失败: {e}"))?;
            serde_yaml::to_string(&value).map_err(|e| format!("序列化配置失败: {e}"))
        }
    }
}

/// 在独立线程上用 current_thread runtime 驱动非 `Send` 的 async 任务。
///
/// rquickjs QuickJS 执行产生的 future 含非 `Send` 结构，不能在 Tauri 命令
/// （要求 `Send` future）中直接 `await`；按 `pp-script::worker` 的同类绕行：
/// `spawn_blocking` + 新建 `current_thread` runtime + `block_on`。
async fn run_blocking<F, Fut, T>(make_fut: F) -> Result<T, String>
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T, String>> + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("创建阻塞运行时失败: {e}"))?;
        rt.block_on(make_fut())
    })
    .await
    .map_err(|e| format!("阻塞任务执行失败: {e}"))?
}
