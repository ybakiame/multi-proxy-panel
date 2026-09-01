/**
 * Core (binary) management API types and functions.
 *
 * Aligned with Rust-side `LocalCoreView`.
 */

import { invoke } from "@tauri-apps/api/core";

/** Core source: `downloaded` (downloaded) / `system` (system detected). */
export type CoreSource = "downloaded" | "system";

/** Local core view (aligned with Rust `LocalCoreView`). */
export interface LocalCoreView {
  /** Core type: `singbox` / `mihomo`. */
  core_type: string;
  version: string;
  path: string;
  source: CoreSource;
  /** Whether this is the currently active core (`core_binary` matches). */
  active: boolean;
}

/** List locally available cores (downloaded + system detected, with active flag). */
export function listCores(): Promise<LocalCoreView[]> {
  return invoke<LocalCoreView[]>("list_cores");
}

/** List recent 10 remote releases (GitHub releases). */
export function listRemoteCoreVersions(coreType: string): Promise<string[]> {
  return invoke<string[]>("list_remote_core_versions", { core_type: coreType });
}

/** List downloaded versions for a core type (version directory scan, semver descending). */
export function listDownloadedVersions(coreType: string): Promise<string[]> {
  return invoke<string[]>("list_downloaded_versions", { core_type: coreType });
}

/** Download specified core version and return its view. */
export function downloadCore(coreType: string, version: string): Promise<LocalCoreView> {
  return invoke<LocalCoreView>("download_core", { core_type: coreType, version });
}

/** Set specified path as core binary (validated then written back to client.json). */
export function setActiveCore(path: string): Promise<void> {
  return invoke<void>("set_active_core", { path });
}

/** Delete a downloaded core (system source / currently in-use core cannot be deleted). */
export function deleteCore(path: string): Promise<void> {
  return invoke<void>("delete_core", { path });
}

/** Manually refresh system core detection. */
export function detectSystemCores(): Promise<LocalCoreView[]> {
  return invoke<LocalCoreView[]>("detect_system_cores");
}
