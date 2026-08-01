import { invoke } from "@tauri-apps/api/core";

/**
 * 与 Rust 侧 `src-tauri/src/commands.rs` 的 serde 视图结构对齐。
 *
 * 字段名以命令层实际 `#[serde(rename_all = "camelCase")]` 输出为准，
 * 与任务描述中的 `mitm.hostnames` / `mitm.script_dialect` 嵌套结构不同
 * （实际为扁平化的 `mitm_hostnames` / `mitm_script_dialect`）。
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

/** 把 Tauri 命令的拒绝值规范为可读错误信息。 */
export function toErrorMessage(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  return String(err);
}
