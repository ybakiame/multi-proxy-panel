/**
 * System / platform API types and functions.
 *
 * Aligned with Rust-side `CapabilitiesView`, `PlatformInfoView`, etc.
 */

import { invoke } from "@tauri-apps/api/core";

/** Platform capability matrix (aligned with Rust `CapabilitiesView`). */
export interface Capabilities {
  /** Platform: `android` / `linux` / `windows` / `macos`. */
  os: string;
  /** Whether running on Android (convenience field). */
  is_android: boolean;
  /** Feature capability matrix. */
  capabilities: {
    /** MITM proxy support. */
    mitm: boolean;
    /** System proxy setting takeover. */
    system_proxy: boolean;
    /** Core binary management (download/delete/detect/switch). */
    core_management: boolean;
    /** TUN mode toggle. */
    tun_toggle: boolean;
    /** Remote script/snippet resource subscription. */
    scripts_remote: boolean;
    /** Scheduled tasks (cron script scheduling). */
    cron_tasks: boolean;
  };
}

/** Platform info (aligned with Rust `PlatformInfoView` / `CapabilitiesView`). */
export interface PlatformInfo {
  /** Platform: `android` / `linux` / `windows` / `macos`. */
  os: string;
}

/** Query platform capability matrix (replaces old `platform_info`, provides finer-grained capability judgment). */
export function getCapabilities(): Promise<Capabilities> {
  return invoke<Capabilities>("get_capabilities");
}

/** Query running platform (frontend hides desktop-exclusive toggles: Android uses VpnService for system proxy / MITM / TUN). */
export function platformInfo(): Promise<PlatformInfo> {
  return invoke<PlatformInfo>("platform_info");
}

/**
 * Request system VPN permission (Android only): navigates to `VpnService.prepare` authorization page,
 * resolves on authorization result; rejects when user denies (error contains `vpn_not_authorized`).
 */
export function requestVpnPermission(): Promise<void> {
  return invoke<void>("request_vpn_permission");
}

/**
 * Read last VPN startup failure reason (Android only): backend reads Kotlin
 * `ProxyVpnService.lastError` via `vpn` plugin. Returns `null` when no failure record / non-Android.
 */
export function vpnLastError(): Promise<string | null> {
  return invoke<string | null>("vpn_last_error");
}

/** TUN privilege status (based on current `core_binary`): `authorized` / `needs_auth` / `unsupported:<reason>`. */
export function tunAuthStatus(): Promise<string> {
  return invoke<string>("tun_auth_status");
}

/** Perform TUN privilege escalation (Linux pkexec setcap / macOS setuid / Windows guide admin restart), returns latest status after authorization. */
export function authorizeTun(): Promise<string> {
  return invoke<string>("authorize_tun");
}

/** Whether GPU acceleration rendering is available (determines whether toast uses HeroUI native animation or custom static implementation). */
export function gpuAcceleration(): Promise<boolean> {
  return invoke<boolean>("gpu_acceleration");
}

/** Read toast rendering mode environment variable override (`PP_TOAST_MODE`): `hero` enables HeroUI native toast, other values / unset returns `null` (default self-implemented static toast). */
export function toastModeOverride(): Promise<string | null> {
  return invoke<string | null>("toast_mode_override");
}

/**
 * Probe GitHub access link availability: requests `https://api.github.com/zen` through real fetch pipeline
 * (GitHub proxy prefix / via local proxy), returns `OK（xxx ms）` style string; rejects on failure and propagates error.
 */
export function testGithubProxy(): Promise<string> {
  return invoke<string>("test_github_proxy");
}
