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
