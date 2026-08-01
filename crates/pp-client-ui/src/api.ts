import { invoke } from "@tauri-apps/api/core";

/**
 * 与 Rust 侧 `src-tauri/src/commands.rs` 的 serde 视图结构对齐。
 *
 * 命令层 `*View` 结构体按字段名原样（snake_case）序列化（无 rename_all），
 * 本文件的 TS 类型与其逐字段一致；与任务描述中的 `mitm.hostnames` /
 * `mitm.script_dialect` 嵌套结构不同（实际为扁平化的 `mitm_hostnames` /
 * `mitm_script_dialect`）。
 */

export type CoreType = "singbox" | "mihomo";

export type MitmScriptDialect = "Surge" | "QuantumultX" | "Loon";

export interface ClientConfig {
  data_dir: string;
  hub_url: string;
  sub_token: string;
  /** `CoreType` 的 serde 表示：`singbox` / `mihomo`。 */
  core_type: string;
  core_binary: string;
  mixed_port: number;
  mitm_enabled: boolean;
  mitm_hostnames: string[];
  mitm_script_dialect: string;
  system_proxy_enabled: boolean;
}

export interface ClientStatus {
  core_running: boolean;
  mitm_addr: string | null;
  system_proxy: boolean;
}

export interface TrafficRecord {
  id: string;
  method: string;
  url: string;
  request_headers: [string, string][];
  request_body: string | null;
  response_status: number;
  response_headers: [string, string][];
  response_body: string | null;
  timestamp: string;
  duration_ms: number;
}

/** 远程订阅资源（与 Rust 侧 `RemoteResourceView` 对齐）。 */
export type RemoteKind = "Script" | "Snippet";

export interface RemoteResource {
  name: string;
  url: string;
  /** `Script`（纯 JS 脚本） / `Snippet`（配置片段）。 */
  kind: RemoteKind;
  /** 脚本方言：`Surge` / `QuantumultX` / `Loon`。 */
  dialect: string;
  /** 资源描述（null = 未配置）。 */
  description: string | null;
  /** 更新间隔（秒）。 */
  update_interval_secs: number;
  enabled: boolean;
  /** 用户为模块参数配置的值 `[key, value]`（对应 `#!arguments=` 声明的键；旧清单缺省为空）。 */
  argument_values: [string, string][];
  /** 资源图标 URL（可选；嗅探结果预填）。 */
  icon: string | null;
}

/** `fetch_remotes` 的拉取报告。 */
export interface FetchReport {
  fetched: number;
  scripts: number;
  rewrites: number;
  tasks: number;
  warnings: string[];
}

/** 定时任务视图（与 `TaskScriptView` 对齐，字段为 snake_case 原始序列化）。 */
export interface TaskScriptView {
  name: string;
  cron_expr: string;
  dialect: string;
  enabled: boolean;
  next_run: string | null;
  last_run: string | null;
  last_error: string | null;
}

/** 配置头 `#!key=value` 元数据（与 Rust 侧 `ConfigMetaView` 对齐）。 */
export interface ConfigMetaView {
  name: string | null;
  desc: string | null;
  author: string | null;
  icon: string | null;
  date: string | null;
  category: string | null;
  open_url: string | null;
}

/** `detect_remote` 的嗅探结果（kind/dialect 按后缀判定，meta 为 Snippet 拉取解析的配置头）。 */
export interface DetectRemoteView {
  /** 嗅探出的资源类型（`Script` / `Snippet`；无法识别时为 null）。 */
  kind: string | null;
  /** 嗅探出的脚本方言（`Surge` / `QuantumultX` / `Loon`；无法识别时为 null）。 */
  dialect: string | null;
  /** 配置头元数据（仅 Snippet 且 URL 可访问时返回；拉取失败或非 Snippet 时为 null）。 */
  meta: ConfigMetaView | null;
}

/** `import_config` 的导入摘要。 */
export interface ImportSummary {
  rewrites: number;
  scripts: number;
  tasks: number;
  hostnames: number;
  warnings: string[];
  /** 配置头解析出的元数据（名称/描述等）。 */
  meta: ConfigMetaView;
}

/** 复写模板列表视图（与 Rust 侧 `ProfileView` 对齐）。 */
export interface ProfileView {
  id: string;
  name: string;
  /** 核心类型：`singbox` / `mihomo`。 */
  core_type: CoreType;
  /** 是否启用（同核心类型下最多一条为 true）。 */
  enabled: boolean;
  /** YAML 复写字节数（列表展示用）。 */
  yaml_bytes: number;
  /** JS 复写字节数（列表展示用）。 */
  js_bytes: number;
  /** 远程 YAML 复写 URL（null = 未配置）。 */
  yaml_url: string | null;
  /** 远程 JS 复写 URL（null = 未配置）。 */
  js_url: string | null;
}

/** 复写模板详情视图（含复写内容，与 Rust 侧 `ProfileDetailView` 对齐）。 */
export interface ProfileDetailView {
  id: string;
  name: string;
  core_type: CoreType;
  enabled: boolean;
  /** YAML 深合并复写（RFC 7386 式；空串 = 未启用）。 */
  yaml_override: string;
  /** JS 复写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。 */
  js_override: string;
  /** 远程 YAML 复写 URL（null = 未配置）。 */
  yaml_url: string | null;
  /** 远程 JS 复写 URL（null = 未配置）。 */
  js_url: string | null;
}

/** 新建复写模板入参。 */
export interface CreateProfileInput {
  name: string;
  core_type: CoreType;
}

/** 更新复写模板入参（YAML/JS 复写与远程 URL 校验失败时被命令层拒绝）。 */
export interface UpdateProfileInput {
  id: string;
  name: string;
  yaml_override: string;
  js_override: string;
  /** 远程 YAML 复写 URL（空串 = 未配置）。 */
  yaml_url: string;
  /** 远程 JS 复写 URL（空串 = 未配置）。 */
  js_url: string;
}

export function getConfig(): Promise<ClientConfig> {
  return invoke<ClientConfig>("get_config");
}

export function saveConfig(cfg: ClientConfig): Promise<void> {
  return invoke<void>("save_config", { cfg });
}

export function startProxy(): Promise<ClientStatus> {
  return invoke<ClientStatus>("start_proxy");
}

export function stopProxy(): Promise<ClientStatus> {
  return invoke<ClientStatus>("stop_proxy");
}

export function proxyStatus(): Promise<ClientStatus> {
  return invoke<ClientStatus>("proxy_status");
}

export function listTraffic(): Promise<TrafficRecord[]> {
  return invoke<TrafficRecord[]>("list_traffic");
}

export function listRemotes(): Promise<RemoteResource[]> {
  return invoke<RemoteResource[]>("list_remotes");
}

export function addRemote(remote: RemoteResource): Promise<void> {
  return invoke<void>("add_remote", { remote });
}

/** 嗅探远端资源 URL：按后缀判定类型/方言，Snippet 可访问时解析配置头元数据。 */
export function detectRemote(url: string): Promise<DetectRemoteView> {
  return invoke<DetectRemoteView>("detect_remote", { url });
}

export function removeRemote(name: string): Promise<void> {
  return invoke<void>("remove_remote", { name });
}

export function fetchRemotes(): Promise<FetchReport> {
  return invoke<FetchReport>("fetch_remotes");
}

export function listTasks(): Promise<TaskScriptView[]> {
  return invoke<TaskScriptView[]>("list_tasks");
}

export function runTask(name: string): Promise<string> {
  return invoke<string>("run_task", { name });
}

export function importConfig(content: string, dialect: string): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_config", { content, dialect });
}

/** 列出全部复写模板。 */
export function listProfiles(): Promise<ProfileView[]> {
  return invoke<ProfileView[]>("list_profiles");
}

/** 新建复写模板（重名报错，错误信息由命令层上抛）。 */
export function createProfile(input: CreateProfileInput): Promise<ProfileView> {
  return invoke<ProfileView>("create_profile", { input });
}

/** 读取单个复写模板详情（含 YAML / JS 复写内容）。 */
export function getProfile(id: string): Promise<ProfileDetailView> {
  return invoke<ProfileDetailView>("get_profile", { id });
}

/** 更新复写模板的可编辑字段（name / yaml_override / js_override）。 */
export function updateProfile(input: UpdateProfileInput): Promise<void> {
  return invoke<void>("update_profile", { input });
}

/** 删除复写模板。 */
export function deleteProfile(id: string): Promise<void> {
  return invoke<void>("delete_profile", { id });
}

/** 切换复写模板启用状态（启用时同核心其他模板自动停用）。 */
export function setProfileEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_profile_enabled", { id, enabled });
}

/** 生成生效配置预览（按当前客户端核心类型；sing-box 为 JSON、mihomo 为 YAML 文本）。 */
export function previewCoreConfig(): Promise<string> {
  return invoke<string>("preview_core_config");
}

/** 订阅用户信息（与 Rust 侧 `SubscriptionUserInfoView` 对齐）。 */
export interface SubscriptionUserInfo {
  /** 已用上行字节数。 */
  upload: number | null;
  /** 已用下行字节数。 */
  download: number | null;
  /** 总流量字节数。 */
  total: number | null;
  /** 到期时间戳（秒）。 */
  expire: number | null;
}

/** 一条订阅的对外视图（与 Rust 侧 `SubscriptionView` 对齐）。 */
export interface SubscriptionView {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  userinfo: SubscriptionUserInfo | null;
  /** 最近一次 fetch 成功的节点数。 */
  node_count: number;
  /** 最近一次 fetch 的错误信息（失败时记录；不阻塞已有数据展示）。 */
  error: string | null;
}

/** 添加订阅的入参。 */
export interface AddSubscriptionInput {
  name: string;
  url: string;
  /** 请求 User-Agent；缺省/空串使用默认 `clash.meta`。 */
  user_agent?: string;
}

export function listSubscriptions(): Promise<SubscriptionView[]> {
  return invoke<SubscriptionView[]>("list_subscriptions");
}

export function addSubscription(input: AddSubscriptionInput): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("add_subscription", { input });
}

export function removeSubscription(id: string): Promise<void> {
  return invoke<void>("remove_subscription", { id });
}

export function setSubscriptionEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke<void>("set_subscription_enabled", { id, enabled });
}

export function refreshSubscription(id: string): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("refresh_subscription", { id });
}

/** 核心来源：`downloaded`（已下载）/ `system`（系统探测）。 */
export type CoreSource = "downloaded" | "system";

/** 本地核心视图（与 Rust 侧 `LocalCoreView` 对齐）。 */
export interface LocalCoreView {
  /** 核心类型：`singbox` / `mihomo`。 */
  core_type: string;
  version: string;
  path: string;
  source: CoreSource;
  /** 是否为当前启用的核心（`core_binary` 匹配）。 */
  active: boolean;
}

/** 列出本地可用核心（已下载 + 系统探测，含 active 标记）。 */
export function listCores(): Promise<LocalCoreView[]> {
  return invoke<LocalCoreView[]>("list_cores");
}

/** 列出远端最近 10 个发布版本（GitHub releases）。 */
export function listRemoteCoreVersions(coreType: string): Promise<string[]> {
  return invoke<string[]>("list_remote_core_versions", { core_type: coreType });
}

/** 下载指定版本核心并返回其视图。 */
export function downloadCore(coreType: string, version: string): Promise<LocalCoreView> {
  return invoke<LocalCoreView>("download_core", { core_type: coreType, version });
}

/** 将指定路径设为核心二进制（校验后写回 client.json）。 */
export function setActiveCore(path: string): Promise<void> {
  return invoke<void>("set_active_core", { path });
}

/** 手动刷新系统核心探测。 */
export function detectSystemCores(): Promise<LocalCoreView[]> {
  return invoke<LocalCoreView[]>("detect_system_cores");
}

/** 把 Tauri 命令的拒绝值规范为可读错误信息。 */
export function toErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}
