/**
 * Core (binary) management API types and functions.
 *
 * Aligned with Rust-side `LocalCoreView`.
 */

import { invoke } from "@tauri-apps/api/core";
import type { CoreChannel } from "../coreChannels";

/** Core source: `downloaded` (downloaded) / `system` (system detected). */
export type CoreSource = "downloaded" | "system";

/** Local core view (aligned with Rust `LocalCoreView`; 仅 sing-box 单核心). */
export interface LocalCoreView {
  version: string;
  path: string;
  source: CoreSource;
  /** Whether this is the currently active core (`core_binary` matches). */
  active: boolean;
}

/** Remote channel latest version view (aligned with Rust `RemoteChannelView`). */
export interface RemoteCoreChannel {
  channel: CoreChannel;
  version: string;
}

/** List locally available cores (downloaded + system detected, with active flag). */
export function listCores(): Promise<LocalCoreView[]> {
  return invoke<LocalCoreView[]>("list_cores");
}

/** List the latest remote version per release channel (稳定版 / 测试版 / 预发布版). */
export function listRemoteCoreChannels(): Promise<RemoteCoreChannel[]> {
  return invoke<RemoteCoreChannel[]>("list_remote_core_channels");
}

/** List downloaded versions (version directory scan, semver descending). */
export function listDownloadedVersions(): Promise<string[]> {
  return invoke<string[]>("list_downloaded_versions");
}

/** Download specified core version and return its view. */
export function downloadCore(version: string): Promise<LocalCoreView> {
  return invoke<LocalCoreView>("download_core", { version });
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
