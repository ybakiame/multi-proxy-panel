/**
 * MITM / traffic / config API functions.
 *
 * Aligned with Rust-side `TrafficRecord`, `MitmCaView`, etc.
 */

import { invoke } from "@tauri-apps/api/core";
import type { ClientConfig, ClientStatus, SaveConfigView } from "./types";

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

/** MITM CA certificate view (aligned with Rust `MitmCaView`). */
export interface MitmCaView {
  /** Absolute path of `ca.crt` (for importing into system/browser trust store). */
  path: string;
  /** PEM format root certificate content. */
  pem: string;
}

/** MITM CA 系统信任状态（aligned with Rust `CaTrustStatus`，serde snake_case）。 */
export type MitmCaTrustStatus = "trusted" | "not_trusted" | "unknown";

/** MITM CA 信任状态视图（aligned with Rust `CaTrustView`）。 */
export interface MitmCaTrustView {
  status: MitmCaTrustStatus;
  /** 状态说明（面向用户的一行描述）。 */
  detail: string;
}

export function getConfig(): Promise<ClientConfig> {
  return invoke<ClientConfig>("get_config");
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

/** Set rule mode (`rule` / `global` / `direct`): persisted to client.json, best-effort hot-switch when core is running and Clash API is enabled. Returns latest status. */
export function setRuleMode(mode: string): Promise<ClientStatus> {
  return invoke<ClientStatus>("set_rule_mode", { mode });
}

export function listTraffic(): Promise<TrafficRecord[]> {
  return invoke<TrafficRecord[]>("list_traffic");
}

/** Get MITM CA certificate (auto-generated if not exists), for client trust guidance display. */
export function getMitmCa(): Promise<MitmCaView> {
  return invoke<MitmCaView>("get_mitm_ca");
}

/** Derived hostname entry: hostname + contributing sources (remote names / 「本地导入」). */
export interface DerivedHostname {
  hostname: string;
  sources: string[];
}

/** MITM whitelist view: script/rewrite-derived (read-only) + manual config (advanced supplement). */
export interface MitmWhitelistView {
  derived: DerivedHostname[];
  manual: string[];
  excluded: string[];
}

/** Get the MITM whitelist view (derived part mirrors the effective runtime scope). */
export function getMitmWhitelist(): Promise<MitmWhitelistView> {
  return invoke<MitmWhitelistView>("get_mitm_whitelist");
}

/** Detect whether the MITM CA is trusted by the system trust store (never rejects on detection failure — status becomes "unknown"). */
export function getMitmCaTrustStatus(): Promise<MitmCaTrustView> {
  return invoke<MitmCaTrustView>("mitm_ca_trust_status");
}

/** Install MITM CA into the system trust store (macOS/Linux 弹系统授权框；Windows 免管理员). Returns the trust status after install. */
export function installMitmCa(): Promise<MitmCaTrustView> {
  return invoke<MitmCaTrustView>("install_mitm_ca");
}
