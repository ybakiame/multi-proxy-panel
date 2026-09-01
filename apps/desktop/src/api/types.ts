/**
 * Core type and base configuration types.
 *
 * Aligned with Rust-side serde view structures in `src-tauri/src/commands.rs`.
 */

export type CoreType = "singbox" | "mihomo";

export type MitmScriptDialect = "Surge" | "Loon";

export interface ClientConfig {
  data_dir: string;
  hub_url: string;
  sub_token: string;
  /** Active subscription id selected on home page (`null` = not selected). */
  active_subscription_id: string | null;
  /** `CoreType` serde representation: `singbox` / `mihomo`. */
  core_type: string;
  core_binary: string;
  mixed_port: number;
  mitm_enabled: boolean;
  mitm_hostnames: string[];
  mitm_script_dialect: string;
  system_proxy_enabled: boolean;
  /** Whether TUN virtual network card is enabled (requires admin/root privileges). */
  tun_enabled: boolean;
  /** TUN protocol stack: `mixed` / `gvisor` / `system`. */
  tun_stack: string;
  /** TUN auto route. */
  tun_auto_route: boolean;
  /** Whether Clash dashboard API is enabled. */
  clash_api_enabled: boolean;
  /** Clash dashboard API listen port. */
  clash_api_port: number;
  /** Clash dashboard API secret (empty = no auth). */
  clash_api_secret: string;
  /** Clash dashboard UI selection: `yacd` / `zashboard` / `metacubexd` (default `zashboard`). */
  clash_api_ui: string;
  /** GitHub proxy prefix (e.g. `https://gh-proxy.com`; empty = direct GitHub). */
  github_proxy_prefix: string;
  /** Whether remote resource fetching goes through local core mixed port (`http://127.0.0.1:{mixed_port}`). */
  fetch_via_local_proxy: boolean;
  /** Rule mode: `rule` / `global` / `direct` (default `rule`). */
  rule_mode: string;
}

export interface ClientStatus {
  core_running: boolean;
  mitm_addr: string | null;
  system_proxy: boolean;
  /** Current effective rule mode: `rule` / `global` / `direct`. */
  rule_mode: string;
  /** Number of rules in the composed config (0 when not running). */
  rule_count: number;
  /** Clash dashboard API address when core is running and Clash API is enabled, otherwise `null`. */
  clash_api_url: string | null;
}

/** `save_config` return view (carries non-blocking warnings). */
export interface SaveConfigView {
  /** Non-blocking warnings (empty hub_url/sub_token, core_type linkage missing local core, etc.). */
  warning: string | null;
}
