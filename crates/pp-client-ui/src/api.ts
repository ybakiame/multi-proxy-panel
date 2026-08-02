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

export type MitmScriptDialect = "Surge" | "Loon";

export interface ClientConfig {
  data_dir: string;
  hub_url: string;
  sub_token: string;
  /** 首页选中的生效订阅 id（`null` = 未选中）。 */
  active_subscription_id: string | null;
  /** `CoreType` 的 serde 表示：`singbox` / `mihomo`。 */
  core_type: string;
  core_binary: string;
  mixed_port: number;
  mitm_enabled: boolean;
  mitm_hostnames: string[];
  mitm_script_dialect: string;
  system_proxy_enabled: boolean;
  /** 是否启用 TUN 虚拟网卡（需管理员/root 权限）。 */
  tun_enabled: boolean;
  /** TUN 协议栈：`mixed` / `gvisor` / `system`。 */
  tun_stack: string;
  /** TUN 自动路由。 */
  tun_auto_route: boolean;
  /** 是否启用 Clash 面板 API。 */
  clash_api_enabled: boolean;
  /** Clash 面板 API 监听端口。 */
  clash_api_port: number;
  /** Clash 面板 API 密钥（空串 = 不鉴权）。 */
  clash_api_secret: string;
  /** Clash 面板 UI 选择：`yacd` / `zashboard` / `metacubexd`（默认 `zashboard`）。 */
  clash_api_ui: string;
  /** GitHub 代理前缀（如 `https://gh-proxy.com`；空串 = 直连 GitHub）。 */
  github_proxy_prefix: string;
  /** 远程资源拉取是否经本地核心 mixed 端口（`http://127.0.0.1:{mixed_port}`）代理。 */
  fetch_via_local_proxy: boolean;
  /** 规则模式：`rule` / `global` / `direct`（默认 `rule`）。 */
  rule_mode: string;
}

export interface ClientStatus {
  core_running: boolean;
  mitm_addr: string | null;
  system_proxy: boolean;
  /** 当前生效的规则模式：`rule` / `global` / `direct`。 */
  rule_mode: string;
  /** 本次合成配置的规则条数（未运行时为 0）。 */
  rule_count: number;
  /** Clash 面板 API 地址（核心运行中且已启用 Clash API 时，否则 `null`）。 */
  clash_api_url: string | null;
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

/** MITM CA 证书视图（与 Rust 侧 `MitmCaView` 对齐）。 */
export interface MitmCaView {
  /** `ca.crt` 的绝对路径（供用户导入系统/浏览器信任库）。 */
  path: string;
  /** PEM 格式的根证书内容。 */
  pem: string;
}

/** 远程订阅资源（与 Rust 侧 `RemoteResourceView` 对齐）。 */
export type RemoteKind = "Script" | "Snippet";

export interface RemoteResource {
  name: string;
  url: string;
  /** `Script`（纯 JS 脚本） / `Snippet`（配置片段）。 */
  kind: RemoteKind;
  /** 脚本方言：`Surge` / `Loon`。 */
  dialect: string;
  /** 资源描述（null = 未配置）。 */
  description: string | null;
  /** 更新间隔（秒）。 */
  update_interval_secs: number;
  enabled: boolean;
  /** 用户为模块参数配置的值 `[key, value]`（对应 `#!arguments=` 声明的键）。 */
  argument_values: [string, string][];
  /** 模块参数声明（`#!arguments=` / Loon `[Argument]` 段；旧数据缺省为空）。 */
  arguments?: ArgSpecView[];
  /** 资源图标 URL（null = 未配置）。 */
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

/** 模块参数声明（`#!arguments=` / Loon `[Argument]` 段，与 Rust 侧 `ArgSpecView` 对齐）。 */
export interface ArgSpecView {
  key: string;
  default_value: string;
  description: string | null;
  /** 控件类型：`Input`（文本输入）/ `Select`（下拉选择）。 */
  kind: "Input" | "Select";
  /** `Select` 控件的可选项。 */
  options: string[];
  /** 参数分组标签（无分组时为 null）。 */
  tag: string | null;
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
  /** 模块参数声明（`#!arguments=` / `#!arguments-desc=`；无声明时为空列表）。 */
  arguments: ArgSpecView[];
}

/** `detect_remote` 的嗅探结果（kind/dialect 按后缀判定，meta 为 Snippet 拉取解析的配置头）。 */
export interface DetectRemoteView {
  /** 嗅探出的资源类型（`Script` / `Snippet`；无法识别时为 null）。 */
  kind: string | null;
  /** 嗅探出的脚本方言（`Surge` / `Loon`；无法识别时为 null）。 */
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

/** 覆写模板列表视图（与 Rust 侧 `ProfileView` 对齐）。 */
export interface ProfileView {
  id: string;
  name: string;
  /** 核心类型：`singbox` / `mihomo`。 */
  core_type: CoreType;
  /** YAML 覆写字节数（列表展示用）。 */
  yaml_bytes: number;
  /** JS 覆写字节数（列表展示用）。 */
  js_bytes: number;
  /** 远程 YAML 覆写 URL（null = 未配置）。 */
  yaml_url: string | null;
  /** 远程 JS 覆写 URL（null = 未配置）。 */
  js_url: string | null;
}

/** 覆写模板详情视图（含覆写内容，与 Rust 侧 `ProfileDetailView` 对齐）。 */
export interface ProfileDetailView {
  id: string;
  name: string;
  core_type: CoreType;
  /** YAML 深合并覆写（RFC 7386 式；空串 = 未启用）。 */
  yaml_override: string;
  /** JS 覆写（同步纯函数 `function main(config){...; return config}`；空串 = 未启用）。 */
  js_override: string;
  /** 远程 YAML 覆写 URL（null = 未配置）。 */
  yaml_url: string | null;
  /** 远程 JS 覆写 URL（null = 未配置）。 */
  js_url: string | null;
}

/** 新建覆写模板入参。 */
export interface CreateProfileInput {
  name: string;
  core_type: CoreType;
}

/** 更新覆写模板入参（YAML/JS 覆写与远程 URL 校验失败时被命令层拒绝）。 */
export interface UpdateProfileInput {
  id: string;
  name: string;
  yaml_override: string;
  js_override: string;
  /** 远程 YAML 覆写 URL（空串 = 未配置）。 */
  yaml_url: string;
  /** 远程 JS 覆写 URL（空串 = 未配置）。 */
  js_url: string;
}

export function getConfig(): Promise<ClientConfig> {
  return invoke<ClientConfig>("get_config");
}

/** `save_config` 的返回视图（携带非阻塞提示）。 */
export interface SaveConfigView {
  /** 非阻塞提示（空 hub_url/sub_token、core_type 联动后缺少本地核心等）。 */
  warning: string | null;
}

export function saveConfig(cfg: ClientConfig): Promise<SaveConfigView> {
  return invoke<SaveConfigView>("save_config", { cfg });
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

/** 设置规则模式（`rule` / `global` / `direct`）：持久化到 client.json，核心运行中且 Clash API 开启时 best-effort 热切换。返回最新运行状态。 */
export function setRuleMode(mode: string): Promise<ClientStatus> {
  return invoke<ClientStatus>("set_rule_mode", { mode });
}

export function listTraffic(): Promise<TrafficRecord[]> {
  return invoke<TrafficRecord[]>("list_traffic");
}

/** 获取 MITM CA 证书（不存在时自动生成），供客户端信任指引展示。 */
export function getMitmCa(): Promise<MitmCaView> {
  return invoke<MitmCaView>("get_mitm_ca");
}

export function listRemotes(): Promise<RemoteResource[]> {
  return invoke<RemoteResource[]>("list_remotes");
}

export function addRemote(remote: RemoteResource): Promise<void> {
  return invoke<void>("add_remote", { remote });
}

/** 按 name 定位全量更新一条远程资源（替代「删除重加」，保留既有缓存）。 */
export function updateRemote(resource: RemoteResource): Promise<void> {
  return invoke<void>("update_remote", { resource });
}

/** 嗅探远端资源 URL：按后缀判定类型/方言，Snippet 可访问时解析配置头元数据。 */
export function detectRemote(url: string): Promise<DetectRemoteView> {
  return invoke<DetectRemoteView>("detect_remote", { url });
}

/** 读取远程资源本地图标缓存（data URL；未缓存 / 读取失败返回 null，前端回退远程 URL）。 */
export function getRemoteIcon(name: string): Promise<string | null> {
  return invoke<string | null>("get_remote_icon", { name });
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

/** 列出全部覆写模板。 */
export function listProfiles(): Promise<ProfileView[]> {
  return invoke<ProfileView[]>("list_profiles");
}

/** 新建覆写模板（重名报错，错误信息由命令层上抛）。 */
export function createProfile(input: CreateProfileInput): Promise<ProfileView> {
  return invoke<ProfileView>("create_profile", { input });
}

/** 读取单个覆写模板详情（含 YAML / JS 覆写内容）。 */
export function getProfile(id: string): Promise<ProfileDetailView> {
  return invoke<ProfileDetailView>("get_profile", { id });
}

/** 更新覆写模板的可编辑字段（name / yaml_override / js_override）。 */
export function updateProfile(input: UpdateProfileInput): Promise<void> {
  return invoke<void>("update_profile", { input });
}

/** 删除覆写模板。 */
export function deleteProfile(id: string): Promise<void> {
  return invoke<void>("delete_profile", { id });
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

/** 订阅内容格式（嗅探结果，与 Rust 侧 `SubFormat` 对齐）。 */
export type SubscriptionFormat = "ShareLinks" | "ClashYaml" | "SingBoxJson";

/** 一条订阅的对外视图（与 Rust 侧 `SubscriptionView` 对齐）。 */
export interface SubscriptionView {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  /** 关联的覆写模板 id（`null` = 不使用覆写）。 */
  profile_id: string | null;
  userinfo: SubscriptionUserInfo | null;
  /** 最近一次 fetch 成功的节点数。 */
  node_count: number;
  /** 最近一次 fetch 的错误信息（失败时记录；不阻塞已有数据展示）。 */
  error: string | null;
  /** 最近一次 fetch 嗅探出的订阅内容格式；未成功拉取时为 undefined。 */
  format?: SubscriptionFormat;
  /** 拉取时使用的请求 User-Agent（缺省/空串 = 默认 clash.meta）。 */
  user_agent?: string;
}

/** 添加订阅的入参。 */
export interface AddSubscriptionInput {
  name: string;
  url: string;
  /** 请求 User-Agent；缺省/空串使用默认 `clash.meta`。 */
  user_agent?: string;
  /** 关联的覆写模板 id（`null` / 空串 = 不使用覆写）。 */
  profile_id?: string | null;
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

/** 设置首页选中的生效订阅（`null` = 清除选中；选中已停用订阅会报错）。 */
export function setActiveSubscription(id: string | null): Promise<void> {
  return invoke<void>("set_active_subscription", { id });
}

export function refreshSubscription(id: string): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("refresh_subscription", { id });
}

/**
 * 更新订阅的 name / url / user_agent / 关联覆写模板（URL 变更时清空上次拉取的缓存）。
 * `profileId`：模板 id = 关联；`null` / 空串 = 取消关联。
 */
export function updateSubscription(
  id: string,
  name: string,
  url: string,
  profileId: string | null,
  userAgent?: string,
): Promise<SubscriptionView> {
  return invoke<SubscriptionView>("update_subscription", { id, name, url, profileId, userAgent });
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

/** 列出指定核心类型已下载的版本（版本目录扫描，语义化版本倒序）。 */
export function listDownloadedVersions(coreType: string): Promise<string[]> {
  return invoke<string[]>("list_downloaded_versions", { core_type: coreType });
}

/** 下载指定版本核心并返回其视图。 */
export function downloadCore(coreType: string, version: string): Promise<LocalCoreView> {
  return invoke<LocalCoreView>("download_core", { core_type: coreType, version });
}

/** 将指定路径设为核心二进制（校验后写回 client.json）。 */
export function setActiveCore(path: string): Promise<void> {
  return invoke<void>("set_active_core", { path });
}

/** 删除一个已下载核心（系统来源 / 当前使用中的核心不可删除）。 */
export function deleteCore(path: string): Promise<void> {
  return invoke<void>("delete_core", { path });
}

/** 手动刷新系统核心探测。 */
export function detectSystemCores(): Promise<LocalCoreView[]> {
  return invoke<LocalCoreView[]>("detect_system_cores");
}

/** TUN 提权状态（基于当前 `core_binary`）：`authorized` / `needs_auth` / `unsupported:<reason>`。 */
export function tunAuthStatus(): Promise<string> {
  return invoke<string>("tun_auth_status");
}

/** 执行 TUN 提权（Linux pkexec setcap / macOS setuid / Windows 引导管理员重启），返回授权后的最新状态。 */
export function authorizeTun(): Promise<string> {
  return invoke<string>("authorize_tun");
}

/** 把 Tauri 命令的拒绝值规范为可读错误信息。 */
export function toErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}
