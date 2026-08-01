//! Tauri 命令层：把 `pp-client` 内部类型包装为 serde 视图结构，供前端调用。
//!
//! 视图类型（`*View`）是独立的 serde 简单结构，避免把内部类型直接暴露给
//! 前端；内部类型与视图的转换通过字段映射与 `serde_json` 往返完成。

use std::path::PathBuf;
use std::sync::Arc;

use pp_client::{
    apply_panel_features, build_core_config_v2, compose_mihomo_config, compose_singbox_config,
    detect_resource_from_url, fetch_subscription_with_ua, infer_core_type, parse_config_meta,
    resolve_remote_overrides, ClientConfig, ClientCoreInventory, ClientState, ConfigMeta,
    EffectiveOverrides, PanelFeatures, Profile, ProfileStoreV2, RemoteKind, RemoteManager,
    RemoteResource, SubContent, Subscription, SubscriptionFetcher, SubscriptionStore,
};
use pp_common::CoreType;
use pp_mitm::TrafficRecorder;
use pp_script::{ScriptDialect, TaskScriptView};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

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
///
/// `#[serde(default)]`：前端回传 payload 缺失任一字段（如旧版前端未发送开关
/// 布尔值）时按默认值补齐，避免整次保存因反序列化失败而中断。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
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
    /// 是否启用 TUN 虚拟网卡（需 root/管理员权限）。
    pub tun_enabled: bool,
    /// TUN 协议栈：`gvisor` / `system` / `mixed`。
    pub tun_stack: String,
    /// TUN 自动路由。
    pub tun_auto_route: bool,
    /// 是否启用 Clash 面板 API。
    pub clash_api_enabled: bool,
    /// Clash 面板 API 监听端口。
    pub clash_api_port: u16,
    /// Clash 面板 API 密钥（空串 = 不鉴权）。
    pub clash_api_secret: String,
    /// Clash 面板 UI 选择：`yacd` / `zashboard` / `metacubexd`（默认 `zashboard`）。
    pub clash_api_ui: String,
}

impl Default for ClientConfigView {
    fn default() -> Self {
        Self {
            data_dir: String::new(),
            hub_url: String::new(),
            sub_token: String::new(),
            core_type: "singbox".to_string(),
            core_binary: String::new(),
            mixed_port: 17890,
            mitm_enabled: true,
            mitm_hostnames: Vec::new(),
            mitm_script_dialect: "Surge".to_string(),
            system_proxy_enabled: false,
            tun_enabled: false,
            tun_stack: "mixed".to_string(),
            tun_auto_route: true,
            clash_api_enabled: false,
            clash_api_port: 9090,
            clash_api_secret: String::new(),
            clash_api_ui: "zashboard".to_string(),
        }
    }
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
            tun_enabled: cfg.tun_enabled,
            tun_stack: cfg.tun_stack.clone(),
            tun_auto_route: cfg.tun_auto_route,
            clash_api_enabled: cfg.clash_api_enabled,
            clash_api_port: cfg.clash_api_port,
            clash_api_secret: cfg.clash_api_secret.clone(),
            clash_api_ui: cfg.clash_api_ui.clone(),
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
            "tun_enabled": self.tun_enabled,
            "tun_stack": self.tun_stack,
            "tun_auto_route": self.tun_auto_route,
            "clash_api_enabled": self.clash_api_enabled,
            "clash_api_port": self.clash_api_port,
            "clash_api_secret": self.clash_api_secret,
            "clash_api_ui": self.clash_api_ui,
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

/// 保存配置的返回视图（携带非阻塞提示）。
#[derive(Debug, Clone, Serialize, Default)]
pub struct SaveConfigView {
    /// 非阻塞提示（core_type 联动后缺少本地核心等）。
    pub warning: Option<String>,
}

/// 保存配置实现（命令层可测试的纯逻辑）。
///
/// - **基本设置可随时保存**：订阅管理已独立（`hub_url` / `sub_token` 字段仅保留
///   兼容，不再校验空值，也不产生缺失提示）；
/// - **core_type 联动本地核心**：`core_type` 变更且当前 `core_binary` 不属于该
///   类型时自动填入该类型首选本地核心（版本最高已下载 → 系统探测）；
///   找不到时保留原路径并返回 `warning` 提示去核心管理下载。
fn save_config_impl(
    data_dir: &std::path::Path,
    cfg: ClientConfigView,
) -> Result<SaveConfigView, String> {
    let mut config = cfg.into_config(data_dir)?;
    let mut warnings: Vec<String> = Vec::new();

    // core_type 联动本地核心二进制。
    let prev_core_type = ClientConfig::load(data_dir).ok().map(|c| c.core_type);
    if prev_core_type != Some(config.core_type) {
        let belongs = !config.core_binary.as_os_str().is_empty()
            && infer_core_type(&config.core_binary) == Some(config.core_type);
        if !belongs {
            let inv = ClientCoreInventory::new(data_dir.to_path_buf());
            match inv.preferred_binary(config.core_type) {
                Some(path) => config.core_binary = path,
                None => warnings.push(format!(
                    "核心类型已切换为 {}，但未找到该类型的本地核心，请到核心管理下载",
                    config.core_type
                )),
            }
        }
    }

    config.save().map_err(|e| format!("保存配置失败: {e}"))?;
    Ok(SaveConfigView {
        warning: if warnings.is_empty() {
            None
        } else {
            Some(warnings.join("；"))
        },
    })
}

/// 保存配置（`hub_url` / `sub_token` 已退役，不再校验也不产生提示；`core_type`
/// 变更时联动本地核心二进制）。
#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    cfg: ClientConfigView,
) -> Result<SaveConfigView, String> {
    save_config_impl(&state.data_dir, cfg)
}

/// 启动代理（无运行状态时先基于已保存配置新建）。
///
/// `ClientState` 注入 [`TauriNotifier`]：脚本 `$notify` / `$notification` 通过
/// tauri-plugin-notification 发送 OS 桌面通知。
///
/// `ClientState::start`（内部 `build_core_config` 的 JS 复写经 pp-script
/// `ScriptWorker` 驱动）返回 `Send` future，可直接在 Tauri 命令（要求 `Send`
/// future）中 `await`。
#[tauri::command]
pub async fn start_proxy(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
) -> Result<ClientStatusView, String> {
    let mut lock = state.client.lock().await;
    if lock.is_none() {
        let cfg = ClientConfig::load(&state.data_dir)
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
    /// 资源描述（可选；`None` = 未配置）。
    pub description: Option<String>,
    pub update_interval_secs: u64,
    pub enabled: bool,
    /// 用户为模块参数配置的值 `(key, value)`（对应 `#!arguments=` 声明的键）。
    pub argument_values: Vec<(String, String)>,
    /// 资源图标 URL（可选；嗅探结果预填）。
    pub icon: Option<String>,
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
            description: remote.description.clone(),
            update_interval_secs: remote.update_interval_secs,
            enabled: remote.enabled,
            argument_values: remote.argument_values.clone(),
            icon: remote.icon.clone(),
        }
    }

    fn into_remote(self) -> Result<RemoteResource, String> {
        let value = serde_json::json!({
            "name": self.name,
            "url": self.url,
            "kind": self.kind,
            "dialect": self.dialect,
            "description": self.description,
            "update_interval_secs": self.update_interval_secs,
            "enabled": self.enabled,
            "argument_values": self.argument_values,
            "icon": self.icon,
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

/// 模块参数声明的对外视图（`#!arguments=` 键/默认值/描述）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ArgSpecView {
    pub key: String,
    pub default_value: String,
    pub description: Option<String>,
}

/// 配置头 `#!key=value` 元数据的对外视图。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfigMetaView {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub author: Option<String>,
    pub icon: Option<String>,
    pub date: Option<String>,
    pub category: Option<String>,
    pub open_url: Option<String>,
    /// 模块参数声明（`#!arguments=` / `#!arguments-desc=`；无声明时为空列表）。
    pub arguments: Vec<ArgSpecView>,
}

impl ConfigMetaView {
    fn from_meta(meta: &pp_client::ConfigMeta) -> Self {
        Self {
            name: meta.name.clone(),
            desc: meta.desc.clone(),
            author: meta.author.clone(),
            icon: meta.icon.clone(),
            date: meta.date.clone(),
            category: meta.category.clone(),
            open_url: meta.open_url.clone(),
            arguments: meta
                .arguments
                .iter()
                .map(|arg| ArgSpecView {
                    key: arg.key.clone(),
                    default_value: arg.default_value.clone(),
                    description: arg.description.clone(),
                })
                .collect(),
        }
    }
}

/// 一次配置导入摘要的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct ImportSummaryView {
    pub rewrites: usize,
    pub scripts: usize,
    pub tasks: usize,
    pub hostnames: usize,
    pub warnings: Vec<String>,
    /// 配置头解析出的元数据（前端展示名称/描述等）。
    pub meta: ConfigMetaView,
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

/// 远程资源类型的字符串表示（与 `RemoteResourceView.kind` 的 serde 表示一致）。
fn remote_kind_str(kind: RemoteKind) -> &'static str {
    match kind {
        RemoteKind::Script => "Script",
        RemoteKind::Snippet => "Snippet",
    }
}

/// 脚本方言的字符串表示（与 `RemoteResourceView.dialect` 的 serde 表示一致）。
fn script_dialect_str(dialect: ScriptDialect) -> &'static str {
    match dialect {
        ScriptDialect::QuantumultX => "QuantumultX",
        ScriptDialect::Surge => "Surge",
        ScriptDialect::Loon => "Loon",
    }
}

/// `detect_remote` 的嗅探结果：后缀判定的类型/方言 + Snippet 拉取解析出的配置头元数据。
#[derive(Debug, Clone, Serialize, Default)]
pub struct DetectRemoteView {
    /// 嗅探出的资源类型（`Script` / `Snippet`；无法识别时为 `None`）。
    pub kind: Option<String>,
    /// 嗅探出的脚本方言（`Surge` / `QuantumultX` / `Loon`；无法识别时为 `None`）。
    pub dialect: Option<String>,
    /// 配置头元数据（仅 Snippet 且 URL 可访问时解析；拉取失败或非 Snippet 时为 `None`）。
    pub meta: Option<ConfigMetaView>,
}

/// 拉取单个 URL 的文本内容（无系统代理，15 秒超时）用于元数据嗅探；任何失败返回 `None`。
async fn fetch_detect_text(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .no_proxy()
        .build()
        .ok()?;
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.text().await.ok()
}

/// 嗅探远端资源 URL：按后缀判定类型/方言；Snippet 且 URL 可访问时拉取内容解析配置头元数据。
///
/// 拉取失败不报错：后缀判定结果（`kind` / `dialect`）照常返回，`meta` 置 `None`，
/// 前端仅用返回字段预填添加表单。
#[tauri::command]
pub async fn detect_remote(url: String) -> Result<DetectRemoteView, String> {
    let url = url.trim();
    let (kind, dialect) = match detect_resource_from_url(url) {
        Some((kind, dialect)) => (
            Some(remote_kind_str(kind).to_string()),
            Some(script_dialect_str(dialect).to_string()),
        ),
        None => (None, None),
    };

    // 仅 Snippet 且 URL 可访问时拉取内容解析元数据；失败静默（meta 置 None）。
    let meta = if kind.as_deref() == Some("Snippet")
        && (url.starts_with("http://") || url.starts_with("https://"))
    {
        fetch_detect_text(url)
            .await
            .map(|text| {
                let parsed = parse_config_meta(&text);
                if parsed != ConfigMeta::default() {
                    Some(ConfigMetaView::from_meta(&parsed))
                } else {
                    None
                }
            })
            .unwrap_or(None)
    } else {
        None
    };

    Ok(DetectRemoteView {
        kind,
        dialect,
        meta,
    })
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
/// 脚本执行由 pp-script 的 `ScriptWorker` 在专有线程驱动（`Send` future），
/// `scheduler.run_now` 的 future 亦为 `Send`，可直接在 Tauri 命令中 `await`。
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
    let output = scheduler
        .run_now(&name)
        .await
        .map_err(|e| format!("运行任务失败: {e}"))?;
    Ok(output.0.to_string())
}

/// 导入 QX / Surge / Loon 配置片段：解析 → 拉取脚本源码回填 → 合并写入本地导入缓存
/// `remote_cache/imported.json`。单个脚本拉取失败记 warning 跳过，不阻塞其他规则合入。
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
    let manager = RemoteManager::new(state.data_dir.clone());
    let summary = manager
        .import_content(&content, script_dialect)
        .await
        .map_err(|e| format!("导入配置失败: {e}"))?;
    Ok(ImportSummaryView {
        rewrites: summary.rewrites,
        scripts: summary.scripts,
        tasks: summary.tasks,
        hostnames: summary.hostnames,
        warnings: summary.warnings,
        meta: ConfigMetaView::from_meta(&summary.meta),
    })
}

/// 核心类型序列化为前端小写约定（`singbox` / `mihomo`）。
fn core_type_str(core_type: CoreType) -> String {
    serde_json::to_value(core_type)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// 解析前端小写核心类型字符串（`singbox` / `mihomo`）。
fn core_type_from_str(s: &str) -> Result<CoreType, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string()))
        .map_err(|_| format!("无效的核心类型 '{s}'（可选: singbox / mihomo）"))
}

/// 解析模板 ID 字符串为 `Uuid`。
fn parse_profile_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("无效的模板 ID: {e}"))
}

/// 复写模板列表视图（与前端 `ProfileView` TS 类型对齐）。
#[derive(Debug, Clone, Serialize)]
pub struct ProfileView {
    pub id: String,
    pub name: String,
    /// 核心类型：`singbox` / `mihomo`（与 `pp_common::CoreType` 的 serde 表示一致）。
    pub core_type: String,
    /// 是否启用（同核心类型下最多一条为 `true`，排他由存储层保证）。
    pub enabled: bool,
    /// YAML 复写字节数（列表展示用）。
    pub yaml_bytes: u64,
    /// JS 复写字节数（列表展示用）。
    pub js_bytes: u64,
    /// 远程 YAML 复写 URL（`null` = 未配置）。
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（`null` = 未配置）。
    pub js_url: Option<String>,
}

impl ProfileView {
    fn from_profile(p: &Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            core_type: core_type_str(p.core_type),
            enabled: p.enabled,
            yaml_bytes: p.yaml_override.len() as u64,
            js_bytes: p.js_override.len() as u64,
            yaml_url: p.yaml_url.clone(),
            js_url: p.js_url.clone(),
        }
    }
}

/// 复写模板详情视图（含完整复写内容，与前端 `ProfileDetailView` TS 类型对齐）。
#[derive(Debug, Clone, Serialize)]
pub struct ProfileDetailView {
    pub id: String,
    pub name: String,
    pub core_type: String,
    pub enabled: bool,
    /// YAML 深合并复写（空串 = 未启用）。
    pub yaml_override: String,
    /// JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。
    pub js_override: String,
    /// 远程 YAML 复写 URL（`null` = 未配置）。
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（`null` = 未配置）。
    pub js_url: Option<String>,
}

impl ProfileDetailView {
    fn from_profile(p: &Profile) -> Self {
        Self {
            id: p.id.to_string(),
            name: p.name.clone(),
            core_type: core_type_str(p.core_type),
            enabled: p.enabled,
            yaml_override: p.yaml_override.clone(),
            js_override: p.js_override.clone(),
            yaml_url: p.yaml_url.clone(),
            js_url: p.js_url.clone(),
        }
    }
}

/// 列出全部复写模板（`data_dir/profiles.json`）。
#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileView>, String> {
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    Ok(profiles.iter().map(ProfileView::from_profile).collect())
}

/// 新建复写模板的入参。
#[derive(Debug, Deserialize)]
pub struct CreateProfileInput {
    pub name: String,
    /// 核心类型：`singbox` / `mihomo`。
    pub core_type: String,
}

/// 新建复写模板；重名时报错（错误信息由存储层上抛）。
#[tauri::command]
pub fn create_profile(
    state: State<'_, AppState>,
    input: CreateProfileInput,
) -> Result<ProfileView, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("模板名称不能为空".to_string());
    }
    let core_type = core_type_from_str(&input.core_type)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profile = store
        .add(&name, core_type)
        .map_err(|e| format!("创建模板失败: {e}"))?;
    Ok(ProfileView::from_profile(&profile))
}

/// 读取单个复写模板详情（含 YAML / JS 复写内容）；模板不存在时报错。
#[tauri::command]
pub fn get_profile(state: State<'_, AppState>, id: String) -> Result<ProfileDetailView, String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    let profile = profiles
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "模板不存在".to_string())?;
    Ok(ProfileDetailView::from_profile(profile))
}

/// 更新复写模板的入参（`core_type` / `enabled` 保持存储值，启用经 `set_profile_enabled` 切换）。
#[derive(Debug, Deserialize)]
pub struct UpdateProfileInput {
    pub id: String,
    pub name: String,
    pub yaml_override: String,
    pub js_override: String,
    /// 远程 YAML 复写 URL（可选；http/https，空串 = 未配置）。
    #[serde(default)]
    pub yaml_url: Option<String>,
    /// 远程 JS 复写 URL（可选；http/https，空串 = 未配置）。
    #[serde(default)]
    pub js_url: Option<String>,
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

/// 校验远程复写 URL：`Some` 且非空时必须是 http:// 或 https:// 开头。
fn validate_remote_url(url: &Option<String>) -> Result<(), String> {
    if let Some(url) = url {
        let url = url.trim();
        if !(url.is_empty() || url.starts_with("http://") || url.starts_with("https://")) {
            return Err("远程复写 URL 必须是 http:// 或 https:// 开头".to_string());
        }
    }
    Ok(())
}

/// 空白串规范化为 `None`（空串 = 未配置）。
fn normalize_optional_url(url: Option<String>) -> Option<String> {
    url.map(|u| u.trim().to_string()).filter(|u| !u.is_empty())
}

/// 更新复写模板的可编辑字段（name / yaml_override / js_override / yaml_url /
/// js_url）；校验失败不落盘。
#[tauri::command]
pub fn update_profile(state: State<'_, AppState>, input: UpdateProfileInput) -> Result<(), String> {
    let id = parse_profile_id(&input.id)?;
    validate_yaml_override(&input.yaml_override)?;
    validate_js_override(&input.js_override)?;
    validate_remote_url(&input.yaml_url)?;
    validate_remote_url(&input.js_url)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    let mut profiles = store.load().map_err(|e| format!("读取复写模板失败: {e}"))?;
    let target = profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or_else(|| "模板不存在".to_string())?;
    target.name = input.name;
    target.yaml_override = input.yaml_override;
    target.js_override = input.js_override;
    target.yaml_url = normalize_optional_url(input.yaml_url);
    target.js_url = normalize_optional_url(input.js_url);
    store
        .save(&profiles)
        .map_err(|e| format!("保存复写模板失败: {e}"))
}

/// 删除复写模板；不存在时报错。
#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    store
        .remove(id)
        .map_err(|e| format!("删除复写模板失败: {e}"))
}

/// 切换复写模板启用状态；启用时同核心类型其他模板自动禁用（排他语义由存储层保证）。
#[tauri::command]
pub fn set_profile_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let id = parse_profile_id(&id)?;
    let store = ProfileStoreV2::new(state.data_dir.clone());
    store
        .set_enabled(id, enabled)
        .map_err(|e| format!("保存模板状态失败: {e}"))
}

/// 生成生效配置预览：拉取订阅 → 内置模板 → 启用模板的远程 + 本地复写叠加 → 核心合成（不含 MITM 链路）。
///
/// 返回最终核心可用的配置文本（sing-box 为 JSON、mihomo 为 YAML），供只读预览。
/// 需要已保存的客户端配置（`data_dir/client.json`，含 hub_url / sub_token / core_type）。
/// 复写取 `ProfileStoreV2::active_profile_for`（当前核心启用模板）：远程复写经
/// `resolve_remote_overrides` 拉取/缓存回退叠加（缓存目录 `data_dir/profile_cache`），
/// 与本地复写一起由 `build_core_config_v2` 合成（与启动实际配置一致）；无启用模板时预览裸模板。
#[tauri::command]
pub async fn preview_core_config(state: State<'_, AppState>) -> Result<String, String> {
    preview_config_async(state.data_dir.clone()).await
}

/// 预览的具体实现（`build_core_config_v2` 的 JS 复写经 pp-script `ScriptWorker`
/// 驱动，future 为 `Send`，可直接在 Tauri 命令中 await）。
async fn preview_config_async(data_dir: std::path::PathBuf) -> Result<String, String> {
    let cfg = ClientConfig::load(&data_dir)
        .map_err(|e| format!("未找到已保存的配置（{e}），请先在设置页保存配置"))?;
    // 远程复写缓存目录（与启动路径一致：`data_dir/profile_cache`）。
    let cache_dir = data_dir.join("profile_cache");
    let store = ProfileStoreV2::new(data_dir);
    // 取启用模板（含远程复写 URL）→ 解析远程复写（拉取/缓存回退/跳过）→
    // 远程为基底、本地覆盖的 v2 构建流程，与启动实际配置保持一致。
    let (effective, warnings) = match store
        .active_profile_for(cfg.core_type)
        .map_err(|e| format!("读取复写模板失败: {e}"))?
    {
        Some(active) => resolve_remote_overrides(&cache_dir, &active).await,
        None => (EffectiveOverrides::default(), Vec::new()),
    };
    for warning in &warnings {
        tracing::warn!(warning, "profile remote override");
    }

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

    let profile_cfg = build_core_config_v2(cfg.core_type, &sub_content, &effective)
        .await
        .map_err(|e| format!("生成配置失败: {e}"))?;

    // 设置页最高优先级的 TUN / Clash 面板配置：compose 之后强制注入，预览与实际
    // 启动路径（state.rs）一致。
    let features = PanelFeatures {
        tun_enabled: cfg.tun_enabled,
        tun_stack: cfg.tun_stack.clone(),
        tun_auto_route: cfg.tun_auto_route,
        clash_api_enabled: cfg.clash_api_enabled,
        clash_api_port: cfg.clash_api_port,
        clash_api_secret: cfg.clash_api_secret.clone(),
        clash_api_ui: cfg.clash_api_ui.clone(),
    };
    let mut value = match cfg.core_type {
        CoreType::SingBox => compose_singbox_config(&profile_cfg, cfg.mixed_port, None)
            .map_err(|e| format!("合成 sing-box 配置失败: {e}"))?,
        CoreType::Mihomo => {
            let yaml =
                serde_yaml::to_string(&profile_cfg).map_err(|e| format!("序列化配置失败: {e}"))?;
            compose_mihomo_config(&yaml, cfg.mixed_port, None)
                .map_err(|e| format!("合成 mihomo 配置失败: {e}"))?
        }
    };
    apply_panel_features(&mut value, cfg.core_type, &features);

    match cfg.core_type {
        CoreType::SingBox => {
            serde_json::to_string_pretty(&value).map_err(|e| format!("序列化配置失败: {e}"))
        }
        CoreType::Mihomo => {
            serde_yaml::to_string(&value).map_err(|e| format!("序列化配置失败: {e}"))
        }
    }
}

// ---------- 核心版本管理 ----------

/// 本地核心的对外视图（与前端 `LocalCoreView` TS 类型对齐）。
#[derive(Debug, Clone, Serialize)]
pub struct LocalCoreView {
    /// 核心类型：`singbox` / `mihomo`。
    pub core_type: String,
    pub version: String,
    pub path: String,
    /// 来源：`downloaded`（已下载）/ `system`（系统探测）。
    pub source: String,
    /// 是否为当前启用的核心（`core_binary` 匹配）。
    pub active: bool,
}

impl LocalCoreView {
    fn from_core(core: &pp_client::LocalCore, active_binary: &std::path::Path) -> Self {
        Self {
            core_type: core_type_str(core.core_type),
            version: core.version.clone(),
            path: core.path.to_string_lossy().into_owned(),
            source: match core.source {
                pp_client::CoreSource::Downloaded => "downloaded",
                pp_client::CoreSource::System => "system",
            }
            .to_string(),
            active: core.path == active_binary,
        }
    }
}

/// 合并已下载与系统探测列表（按路径去重，系统探测优先展示）。
fn merge_cores(
    mut installed: Vec<pp_client::LocalCore>,
    system: Vec<pp_client::LocalCore>,
) -> Vec<pp_client::LocalCore> {
    for s in system {
        if !installed.iter().any(|c| c.path == s.path) {
            installed.push(s);
        }
    }
    installed
}

/// 当前配置中的 active 核心二进制路径（配置未保存时为空）。
fn active_binary(data_dir: &std::path::Path) -> std::path::PathBuf {
    pp_client::ClientConfig::load(data_dir)
        .map(|c| c.core_binary)
        .unwrap_or_default()
}

/// 列出本地可用核心（已下载 + 系统探测，含 active 标记）。
#[tauri::command]
pub async fn list_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let cores = merge_cores(inv.list_installed(), inv.detect_system_cores());
    let active = active_binary(&state.data_dir);
    Ok(cores
        .iter()
        .map(|c| LocalCoreView::from_core(c, &active))
        .collect())
}

/// 列出远端最近 10 个发布版本（GitHub releases，去 `v` 前缀）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_remote_core_versions(
    state: State<'_, AppState>,
    core_type: String,
) -> Result<Vec<String>, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    inv.list_remote_versions(ct)
        .await
        .map_err(|e| format!("拉取远端版本失败: {e}"))
}

/// 列出指定核心类型已下载的版本（版本目录扫描，语义化版本倒序）的具体实现。
fn list_downloaded_versions_impl(
    data_dir: &std::path::Path,
    core_type: String,
) -> Result<Vec<String>, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(data_dir.to_path_buf());
    Ok(inv.list_downloaded_versions(ct))
}

/// 列出指定核心类型已下载的版本（版本目录扫描，语义化版本倒序）。
#[tauri::command(rename_all = "snake_case")]
pub async fn list_downloaded_versions(
    state: State<'_, AppState>,
    core_type: String,
) -> Result<Vec<String>, String> {
    list_downloaded_versions_impl(&state.data_dir, core_type)
}

/// 下载指定版本核心并返回其视图。
#[tauri::command(rename_all = "snake_case")]
pub async fn download_core(
    state: State<'_, AppState>,
    core_type: String,
    version: String,
) -> Result<LocalCoreView, String> {
    let ct = core_type_from_str(&core_type)?;
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let core = inv
        .download(ct, &version)
        .await
        .map_err(|e| format!("下载核心失败: {e}"))?;
    let active = active_binary(&state.data_dir);
    Ok(LocalCoreView::from_core(&core, &active))
}

/// 将指定路径设为核心二进制：校验存在且可执行后，先在本地核心清单
/// （已下载 + 系统探测合并）中按路径匹配其 `core_type`，未命中则按文件名
/// 推断；随后同时写回 `client.json` 的 `core_binary` 与 `core_type`，避免
/// 启用异类型核心时两者不一致。无法识别类型时返回错误提示手动选择。
#[tauri::command(rename_all = "snake_case")]
pub async fn set_active_core(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let bin = PathBuf::from(&path);
    if !bin.is_file() {
        return Err(format!("核心二进制不存在: {path}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(&bin).map_err(|e| format!("读取核心信息失败: {e}"))?;
        if meta.permissions().mode() & 0o111 == 0 {
            return Err(format!("核心二进制不可执行: {path}"));
        }
    }
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let core_type = merge_cores(inv.list_installed(), inv.detect_system_cores())
        .into_iter()
        .find(|c| c.path == bin)
        .map(|c| c.core_type)
        .or_else(|| pp_client::infer_core_type(&bin))
        .ok_or_else(|| format!("无法识别核心类型: {path}，请在设置页手动选择核心类型"))?;
    let mut config = match pp_client::ClientConfig::load(&state.data_dir) {
        Ok(cfg) => cfg,
        Err(_) => pp_client::ClientConfig::new(
            state.data_dir.clone(),
            String::new(),
            String::new(),
            CoreType::SingBox,
            PathBuf::new(),
        ),
    };
    config.core_binary = bin;
    config.core_type = core_type;
    config.save().map_err(|e| format!("保存配置失败: {e}"))
}

/// 手动刷新系统核心探测。
#[tauri::command]
pub async fn detect_system_cores(state: State<'_, AppState>) -> Result<Vec<LocalCoreView>, String> {
    let inv = pp_client::ClientCoreInventory::new(state.data_dir.clone());
    let active = active_binary(&state.data_dir);
    Ok(inv
        .detect_system_cores()
        .iter()
        .map(|c| LocalCoreView::from_core(c, &active))
        .collect())
}

// ---------- 订阅管理 ----------

/// 订阅用户信息的对外视图（与 `pp_client::SubscriptionInfo` 字段对齐）。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SubscriptionUserInfoView {
    /// 已用上行字节数。
    pub upload: Option<u64>,
    /// 已用下行字节数。
    pub download: Option<u64>,
    /// 总流量字节数。
    pub total: Option<u64>,
    /// 到期时间戳（秒）。
    pub expire: Option<u64>,
}

impl SubscriptionUserInfoView {
    fn from_info(info: &pp_client::SubscriptionInfo) -> Self {
        Self {
            upload: info.upload,
            download: info.download,
            total: info.total,
            expire: info.expire,
        }
    }
}

/// 一条订阅的对外视图。
#[derive(Debug, Clone, Serialize)]
pub struct SubscriptionView {
    pub id: String,
    pub name: String,
    pub url: String,
    pub enabled: bool,
    pub userinfo: Option<SubscriptionUserInfoView>,
    /// 最近一次 fetch 成功的节点数（sing-box 侧可用节点数）。
    pub node_count: u64,
    /// 最近一次 fetch 的错误信息（失败时记录；不阻塞已有数据展示）。
    pub error: Option<String>,
}

impl SubscriptionView {
    fn from_sub(sub: &Subscription) -> Self {
        Self {
            id: sub.id.to_string(),
            name: sub.name.clone(),
            url: sub.url.clone(),
            enabled: sub.enabled,
            userinfo: sub
                .userinfo
                .as_ref()
                .map(SubscriptionUserInfoView::from_info),
            node_count: sub.node_count,
            error: sub.error.clone(),
        }
    }
}

/// 添加订阅的入参。
#[derive(Debug, Deserialize)]
pub struct AddSubscriptionInput {
    pub name: String,
    pub url: String,
    /// 请求 User-Agent；`None` / 空串使用默认 `clash.meta`。
    pub user_agent: Option<String>,
}

/// 校验订阅 URL 必须为 http/https。
fn validate_subscription_url(url: &str) -> Result<(), String> {
    if url.starts_with("http://") || url.starts_with("https://") {
        Ok(())
    } else {
        Err("订阅 URL 必须以 http:// 或 https:// 开头".to_string())
    }
}

/// 解析订阅 ID 字符串为 `Uuid`。
fn parse_subscription_id(id: &str) -> Result<Uuid, String> {
    Uuid::parse_str(id).map_err(|e| format!("无效的订阅 ID: {e}"))
}

/// 把一次 fetch 结果合并进订阅（成功更新 userinfo / 节点数并清空 error；
/// 失败仅记录 error，保留旧数据）。拉取 UA 取订阅条目配置（`None` → 默认 clash.meta）。
async fn apply_fetch(sub: &mut Subscription, url: &str) {
    match fetch_subscription_with_ua(url, sub.user_agent.as_deref()).await {
        Ok(result) => {
            sub.userinfo = result.userinfo;
            sub.node_count = result.singbox_nodes.len() as u64;
            sub.error = None;
        }
        Err(e) => {
            sub.error = Some(format!("拉取失败: {e}"));
        }
    }
}

/// 将更新后的订阅按 id 写回存储。
fn update_subscription(store: &SubscriptionStore, sub: &Subscription) -> Result<(), String> {
    let mut subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    if let Some(existing) = subs.iter_mut().find(|s| s.id == sub.id) {
        *existing = sub.clone();
    }
    store.save(&subs).map_err(|e| format!("保存订阅失败: {e}"))
}

/// 列出全部订阅（含最近一次 fetch 的节点数与错误信息）。
#[tauri::command]
pub async fn list_subscriptions(
    state: State<'_, AppState>,
) -> Result<Vec<SubscriptionView>, String> {
    let store = SubscriptionStore::new(state.data_dir.clone());
    let subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    Ok(subs.iter().map(SubscriptionView::from_sub).collect())
}

/// 添加订阅：校验 URL → 落盘（默认启用）→ 立即 fetch 一次拿 userinfo + 节点数。
///
/// fetch 失败不阻塞添加，错误记入返回视图的 `error` 字段。
#[tauri::command]
pub async fn add_subscription(
    state: State<'_, AppState>,
    input: AddSubscriptionInput,
) -> Result<SubscriptionView, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let url = input.url.trim().to_string();
    validate_subscription_url(&url)?;
    // 空串 / 纯空白 UA 归一化为 None（使用默认 clash.meta）。
    let ua = input
        .user_agent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let store = SubscriptionStore::new(state.data_dir.clone());
    let mut sub = store
        .add(&name, &url, true, ua)
        .map_err(|e| format!("保存订阅失败: {e}"))?;
    apply_fetch(&mut sub, &url).await;
    update_subscription(&store, &sub)?;
    Ok(SubscriptionView::from_sub(&sub))
}

/// 刷新订阅：重新 fetch 更新 userinfo / 节点数。
///
/// 失败时保留旧数据并返回（视图 `error` 字段携带错误信息）。
#[tauri::command]
pub async fn refresh_subscription(
    state: State<'_, AppState>,
    id: String,
) -> Result<SubscriptionView, String> {
    let id = parse_subscription_id(&id)?;
    let store = SubscriptionStore::new(state.data_dir.clone());
    let mut subs = store.load().map_err(|e| format!("读取订阅失败: {e}"))?;
    let idx = subs
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| "订阅不存在".to_string())?;
    let url = subs[idx].url.clone();
    apply_fetch(&mut subs[idx], &url).await;
    store
        .save(&subs)
        .map_err(|e| format!("保存订阅失败: {e}"))?;
    Ok(SubscriptionView::from_sub(&subs[idx]))
}

/// 删除订阅；不存在时静默返回。
#[tauri::command]
pub async fn remove_subscription(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let id = parse_subscription_id(&id)?;
    let store = SubscriptionStore::new(state.data_dir.clone());
    store.remove(id).map_err(|e| format!("删除订阅失败: {e}"))
}

/// 切换订阅启用状态（取第一个启用的订阅生效，重启代理应用后应用）。
#[tauri::command]
pub async fn set_subscription_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let id = parse_subscription_id(&id)?;
    let store = SubscriptionStore::new(state.data_dir.clone());
    store
        .set_enabled(id, enabled)
        .map_err(|e| format!("保存订阅失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 简易临时目录（测试用，避免新增 tempfile dev-dependency）：
    /// 进程 id + 原子计数器生成唯一路径，Drop 时递归清理。
    struct TestDir(PathBuf);

    impl TestDir {
        fn new() -> Self {
            static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "pp-client-ui-test-{}-{}",
                std::process::id(),
                n
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &std::path::Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// 全局 PATH 锁：环境变量是进程级状态，并行测试间互斥，避免相互串台。
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 在受控 PATH 下执行闭包（互斥串行化；避免宿主环境 PATH 中的真实核心干扰
    /// 系统探测回退）。
    fn with_empty_path<T>(f: impl FnOnce() -> T) -> T {
        let _guard = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let old = std::env::var_os("PATH");
        // Rust 2024 下 std::env 的 set_var 标记为 unsafe（并发修改环境变量是未定义
        // 行为），PATH_LOCK 保证测试进程内串行访问。
        unsafe {
            std::env::set_var("PATH", "/nonexistent-pp-test-bin");
        }
        let result = f();
        match old {
            Some(v) => unsafe { std::env::set_var("PATH", v) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        result
    }

    fn full_view(data_dir: &std::path::Path) -> ClientConfigView {
        ClientConfigView {
            hub_url: "http://127.0.0.1:50052".to_string(),
            sub_token: "tok".to_string(),
            core_type: "singbox".to_string(),
            core_binary: data_dir
                .join("cores/sing-box/1.13.15/sing-box")
                .to_string_lossy()
                .into_owned(),
            ..ClientConfigView::default()
        }
    }

    /// 写入一个「已下载核心」假二进制（`cores/<dir>/<version>/<dir>`）。
    fn write_core(data_dir: &std::path::Path, core_dir: &str, version: &str) {
        let bin = data_dir
            .join("cores")
            .join(core_dir)
            .join(version)
            .join(core_dir);
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"fake core").unwrap();
    }

    // ---------- 项 1 回归：命令层序列化往返 ----------

    #[test]
    fn save_config_roundtrip_preserves_basic_settings_and_panel_features() {
        let dir = TestDir::new();
        let mut view = full_view(dir.path());
        view.mixed_port = 20000;
        view.mitm_enabled = false;
        view.system_proxy_enabled = true;
        view.tun_enabled = true;
        view.tun_stack = "system".to_string();
        view.tun_auto_route = false;
        view.clash_api_enabled = true;
        view.clash_api_port = 9091;
        view.clash_api_secret = "sekret".to_string();
        view.clash_api_ui = "yacd".to_string();

        let result = with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());
        assert!(
            result.warning.is_none(),
            "完整 payload 不应有 warning: {:?}",
            result.warning
        );

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 20000);
        assert!(!saved.mitm_enabled);
        assert!(saved.system_proxy_enabled);
        assert!(saved.tun_enabled);
        assert_eq!(saved.tun_stack, "system");
        assert!(!saved.tun_auto_route);
        assert!(saved.clash_api_enabled);
        assert_eq!(saved.clash_api_port, 9091);
        assert_eq!(saved.clash_api_secret, "sekret");
        assert_eq!(saved.clash_api_ui, "yacd");

        // load → from_config 往返保真（前端下次保存时的 payload 基础）。
        let view2 = ClientConfigView::from_config(&saved);
        assert_eq!(view2.mixed_port, 20000);
        assert!(!view2.mitm_enabled);
        assert!(view2.system_proxy_enabled);
        assert!(view2.tun_enabled);
        assert_eq!(view2.tun_stack, "system");
        assert!(view2.clash_api_enabled);
        assert_eq!(view2.clash_api_secret, "sekret");
        assert_eq!(view2.clash_api_ui, "yacd");
    }

    #[test]
    fn save_config_partial_payload_defaults_and_saves() {
        let dir = TestDir::new();
        // 旧前端 / 部分 payload：缺失 system_proxy_enabled 等布尔字段，且 hub_url /
        // sub_token 为空 —— 缺字段按默认值补齐，空 hub 字段不再产生 warning。
        let json = serde_json::json!({
            "data_dir": "/tmp/pp",
            "hub_url": "",
            "sub_token": "",
            "core_type": "singbox",
            "core_binary": "",
            "mixed_port": 12345,
            "mitm_enabled": false,
            "mitm_hostnames": [],
            "mitm_script_dialect": "Surge",
        });
        let view: ClientConfigView = serde_json::from_value(json).unwrap();
        assert_eq!(view.clash_api_ui, "zashboard", "缺失字段按默认值补齐");
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 12345);
        assert!(!saved.mitm_enabled);
        assert!(!saved.system_proxy_enabled, "缺失布尔字段按默认值补齐");
        assert_eq!(saved.clash_api_ui, "zashboard");

        // 空 hub_url/sub_token 已退役：不再产生订阅缺失提示（core_type 联动警告
        // 属于另一独立路径，与本回归无关）。
        if let Some(w) = &result.warning {
            assert!(
                !w.contains("hub_url") && !w.contains("sub_token"),
                "空 hub_url/sub_token 不应再产生 warning: {w}"
            );
        }
    }

    #[test]
    fn save_config_empty_hub_and_token_saves_without_warning() {
        let dir = TestDir::new();
        let view = full_view(dir.path());

        // 先保存一次（含 hub_url/sub_token），随后用户清空两者保存基本设置改动。
        with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());
        let mut cleared = view;
        cleared.hub_url = String::new();
        cleared.sub_token = String::new();
        cleared.mixed_port = 30000;
        let result = with_empty_path(|| save_config_impl(dir.path(), cleared).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.mixed_port, 30000, "基本设置应保存成功");
        assert!(
            result.warning.is_none(),
            "空 hub_url/sub_token 不应再提示: {:?}",
            result.warning
        );
    }

    // ---------- 项 2 回归：core_type 联动本地核心 ----------

    #[test]
    fn save_config_switching_core_type_auto_fills_preferred_binary() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "mihomo", "1.19.29");
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        // 用户把 core_type 切到 mihomo，但 core_binary 仍指向 sing-box。
        let mut view = full_view(dir.path());
        view.core_type = "mihomo".to_string();
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::Mihomo);
        // 自动联动：填入已下载 mihomo 首选二进制。
        assert_eq!(
            saved.core_binary,
            dir.path().join("cores/mihomo/1.19.29/mihomo")
        );
        assert!(
            result.warning.is_none(),
            "找到本地核心时不应有联动 warning: {:?}",
            result.warning
        );
    }

    #[test]
    fn save_config_switching_core_type_without_local_core_keeps_and_warns() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let mut view = full_view(dir.path());
        view.core_type = "mihomo".to_string();
        let result = with_empty_path(|| save_config_impl(dir.path(), view).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::Mihomo);
        // 找不到 mihomo 本地核心 → 保留原二进制并返回 warning。
        assert_eq!(
            saved.core_binary,
            dir.path().join("cores/sing-box/1.13.15/sing-box")
        );
        let warning = result.warning.expect("应返回联动 warning");
        assert!(warning.contains("核心类型已切换"), "{warning}");
    }

    #[test]
    fn save_config_same_core_type_keeps_binary_untouched() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        let prev = ClientConfig::new(
            dir.path().to_path_buf(),
            "http://127.0.0.1:50052",
            "tok",
            CoreType::SingBox,
            dir.path().join("cores/sing-box/1.13.15/sing-box"),
        );
        prev.save().unwrap();

        let view = full_view(dir.path());
        with_empty_path(|| save_config_impl(dir.path(), view.clone()).unwrap());

        let saved = ClientConfig::load(dir.path()).unwrap();
        assert_eq!(saved.core_type, CoreType::SingBox);
        // 未切换 core_type → 二进制保持原样（不触发联动）。
        assert_eq!(
            saved.core_binary,
            dir.path().join("cores/sing-box/1.13.15/sing-box")
        );
    }

    // ---------- 项 2：list_downloaded_versions 命令层 ----------

    #[test]
    fn list_downloaded_versions_lists_semantic_descending() {
        let dir = TestDir::new();
        write_core(dir.path(), "sing-box", "1.13.15");
        write_core(dir.path(), "sing-box", "1.14.0-beta.4");
        write_core(dir.path(), "sing-box", "1.14.0");
        write_core(dir.path(), "mihomo", "1.19.29");

        // 语义化倒序：1.14.0 > 1.14.0-beta.4 > 1.13.15。
        let versions =
            list_downloaded_versions_impl(dir.path(), "singbox".to_string()).unwrap();
        assert_eq!(versions, vec!["1.14.0", "1.14.0-beta.4", "1.13.15"]);

        let mihomo = list_downloaded_versions_impl(dir.path(), "mihomo".to_string()).unwrap();
        assert_eq!(mihomo, vec!["1.19.29"]);

        // 无效 core_type 字符串报错。
        assert!(list_downloaded_versions_impl(dir.path(), "bogus".to_string()).is_err());
    }
}
