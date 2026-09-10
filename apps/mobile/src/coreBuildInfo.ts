/**
 * panelcore.aar（sing-box libbox）编译期元数据。
 *
 * - sing-box 版本：经运行时 API `Libbox.version()` 动态读取（@pp/client-core
 *   `useCoreVersion` → Rust `core_version` → Kotlin `VpnPlugin.coreVersion`），
 *   无需在此硬编码。
 * - 编译 tags：libbox 无运行时导出（仅有 `Version()`，无 `Tags()`），只能硬编码。
 *
 * 同步提醒：升级 sing-box 或调整构建 tags 后，请同步更新本常量与
 * `apps/mobile/scripts/build-panel-core.sh` 的 `-tags`（两者必须一致）。
 */
export const SING_BOX_BUILD_TAGS = [
  "with_gvisor",
  "with_quic",
  "with_wireguard",
  "with_utls",
  "with_naive_outbound",
  "with_clash_api",
  "with_usbip",
  "with_openvpn",
  "with_openconnect",
  "badlinkname",
  "tfogo_checklinkname0",
  "with_tailscale",
  "ts_omit_logtail",
  "ts_omit_ssh",
  "ts_omit_drive",
  "ts_omit_taildrop",
  "ts_omit_webclient",
  "ts_omit_doctor",
  "ts_omit_capture",
  "ts_omit_kube",
  "ts_omit_aws",
  "ts_omit_synology",
  "ts_omit_bird",
] as const;
