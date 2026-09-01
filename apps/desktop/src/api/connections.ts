/**
 * Connection (Clash API) API types and functions.
 *
 * Aligned with Rust-side `ConnectionView`, `ActiveConnections`.
 */

import { invoke } from "@tauri-apps/api/core";

/** Connection view (aligned with Rust `ConnectionView`). */
export interface ConnectionView {
  id: string;
  host: string;
  network: string;
  chain: string;
  rule: string;
  rule_payload: string;
  upload: number;
  download: number;
  start: number;
}

/** Active connections summary (aligned with Rust `ActiveConnections`). */
export interface ActiveConnections {
  connections: ConnectionView[];
  upload_total: number;
  download_total: number;
}

/** Get active connection list (returns error when core is not running). */
export function connectionsActive(): Promise<ActiveConnections> {
  return invoke<ActiveConnections>("connections_active");
}

/** Get closed connection records (returns error when core is not running). */
export function connectionsClosed(): Promise<ConnectionView[]> {
  return invoke<ConnectionView[]>("connections_closed");
}

/** Close a connection by ID. */
export function connectionsClose(id: string): Promise<void> {
  return invoke<void>("connections_close", { id });
}
