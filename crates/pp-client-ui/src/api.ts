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
  /** 更新间隔（秒）。 */
  update_interval_secs: number;
  enabled: boolean;
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

/** `import_config` 的导入摘要。 */
export interface ImportSummary {
  rewrites: number;
  scripts: number;
  tasks: number;
  hostnames: number;
  warnings: string[];
}

/** Profile 复写配置（与 Rust 侧 `ProfileOverridesView` 对齐；空串 = 未启用）。 */
export interface ProfileOverrides {
  /** YAML 深合并复写（RFC 7386 式）。 */
  yaml_override: string;
  /** JS 复写（同步纯函数 `function main(config){...; return config}`）。 */
  js_override: string;
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

/** 读取 Profile 复写配置（不存在时返回空串默认）。 */
export function getProfileOverrides(): Promise<ProfileOverrides> {
  return invoke<ProfileOverrides>("get_profile_overrides");
}

/** 保存 Profile 复写配置（YAML/JS 校验失败时被命令层拒绝）。 */
export function saveProfileOverrides(overrides: ProfileOverrides): Promise<void> {
  return invoke<void>("save_profile_overrides", { ov: overrides });
}

/** 生成生效配置预览（sing-box 为 JSON、mihomo 为 YAML 文本）。 */
export function previewCoreConfig(): Promise<string> {
  return invoke<string>("preview_core_config");
}

/** 把 Tauri 命令的拒绝值规范为可读错误信息。 */
export function toErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}
