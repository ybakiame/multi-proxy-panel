/**
 * Remote resource (script / snippet) API types and functions.
 *
 * Aligned with Rust-side `RemoteResourceView`, `FetchReport`, etc.
 */

import { invoke } from "@tauri-apps/api/core";

export type RemoteKind = "Script" | "Snippet";

export interface RemoteResource {
  name: string;
  url: string;
  /** `Script` (pure JS script) / `Snippet` (config snippet). */
  kind: RemoteKind;
  /** Script dialect: `Surge` / `Loon`. */
  dialect: string;
  /** Resource description (`null` = not configured). */
  description: string | null;
  /** Update interval (seconds). */
  update_interval_secs: number;
  enabled: boolean;
  /** User-configured module parameter values `[key, value]` (corresponding to `#!arguments=` declared keys). */
  argument_values: [string, string][];
  /** Module parameter declarations (`#!arguments=` / Loon `[Argument]` section; empty for legacy data). */
  arguments?: ArgSpecView[];
  /** Resource icon URL (`null` = not configured). */
  icon: string | null;
}

/** `fetch_remotes` fetch report. */
export interface FetchReport {
  fetched: number;
  scripts: number;
  rewrites: number;
  tasks: number;
  warnings: string[];
}

/** Module parameter declaration (`#!arguments=` / Loon `[Argument]` section, aligned with Rust `ArgSpecView`). */
export interface ArgSpecView {
  key: string;
  default_value: string;
  description: string | null;
  /** Control type: `Input` (text input) / `Select` (dropdown). */
  kind: "Input" | "Select";
  /** Options for `Select` control. */
  options: string[];
  /** Parameter group tag (`null` when no group). */
  tag: string | null;
}

/** Config header `#!key=value` metadata (aligned with Rust `ConfigMetaView`). */
export interface ConfigMetaView {
  name: string | null;
  desc: string | null;
  author: string | null;
  icon: string | null;
  date: string | null;
  category: string | null;
  open_url: string | null;
  /** Module parameter declarations (`#!arguments=` / `#!arguments-desc=`; empty when none). */
  arguments: ArgSpecView[];
}

/** `detect_remote` sniff result (kind/dialect determined by suffix, meta is Snippet fetch-parsed config header). */
export interface DetectRemoteView {
  /** Sniffed resource type (`Script` / `Snippet`; `null` when unrecognized). */
  kind: string | null;
  /** Sniffed script dialect (`Surge` / `Loon`; `null` when unrecognized). */
  dialect: string | null;
  /** Config header metadata (only for Snippet when URL is accessible; `null` on fetch failure or non-Snippet). */
  meta: ConfigMetaView | null;
}

/** `import_config` import summary. */
export interface ImportSummary {
  rewrites: number;
  scripts: number;
  tasks: number;
  hostnames: number;
  warnings: string[];
  /** Metadata parsed from config header (name/description, etc.). */
  meta: ConfigMetaView;
}

export function listRemotes(): Promise<RemoteResource[]> {
  return invoke<RemoteResource[]>("list_remotes");
}

export function addRemote(remote: RemoteResource): Promise<void> {
  return invoke<void>("add_remote", { remote });
}

/** Update a remote resource by name (replaces "delete then add", preserves existing cache). */
export function updateRemote(resource: RemoteResource): Promise<void> {
  return invoke<void>("update_remote", { resource });
}

/** Sniff remote resource URL: determine type/dialect by suffix, parse config header metadata for accessible Snippet. */
export function detectRemote(url: string): Promise<DetectRemoteView> {
  return invoke<DetectRemoteView>("detect_remote", { url });
}

/** Read local icon cache for a remote resource (data URL; returns `null` when not cached / read fails, frontend falls back to remote URL). */
export function getRemoteIcon(name: string): Promise<string | null> {
  return invoke<string | null>("get_remote_icon", { name });
}

export function removeRemote(name: string): Promise<void> {
  return invoke<void>("remove_remote", { name });
}

export function fetchRemotes(): Promise<FetchReport> {
  return invoke<FetchReport>("fetch_remotes");
}

export function importConfig(content: string, dialect: string): Promise<ImportSummary> {
  return invoke<ImportSummary>("import_config", { content, dialect });
}
