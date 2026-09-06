/**
 * Proxy (agent page) API types and functions.
 *
 * Aligned with Rust-side `GroupView`, `NodeView`, `ProxyList`, `DelayResult`.
 */

import { invoke } from "@tauri-apps/api/core";

/** Proxy group view (aligned with Rust `GroupView`). */
export interface GroupView {
  name: string;
  /** Group type, e.g. `Selector`, `URLTest`, `Fallback`, `LoadBalance`. */
  group_type: string;
  /** Currently selected member node name. */
  now: string;
  /** Member node name list. */
  members: string[];
}

/** Proxy node view (aligned with Rust `NodeView`). */
export interface NodeView {
  name: string;
  /** Node type, e.g. `Shadowsocks`, `Vmess`, `Trojan`. */
  node_type: string;
  /** Latest speed test delay (ms); `null` = not tested or timed out. */
  delay_ms: number | null;
  /** Whether UDP is supported. */
  udp: boolean;
}

/** Proxy list (aligned with Rust `ProxyList`). */
export interface ProxyList {
  groups: GroupView[];
  nodes: NodeView[];
}

/** Single node delay test result (aligned with Rust `DelayResult`). */
export interface DelayResult {
  name: string;
  delay_ms: number | null;
}

/** List all proxy groups and nodes (returns error when core is not running). */
export function proxiesList(): Promise<ProxyList> {
  return invoke<ProxyList>("proxies_list");
}

/** Select node in a group (persisted to client.json). */
export function proxiesSelect(group: string, name: string): Promise<void> {
  return invoke<void>("proxies_select", { group, name });
}

/** Test delay of a single node (`null` = failure or timeout). */
export function proxiesTestDelay(name: string): Promise<number | null> {
  return invoke<number | null>("proxies_test_delay", { name });
}

/** Test delay of all members in a group (backend concurrent throttling). */
export function proxiesTestGroup(group: string): Promise<DelayResult[]> {
  return invoke<DelayResult[]>("proxies_test_group", { group });
}
