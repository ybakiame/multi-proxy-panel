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
 *
 * 注：上游 1.15 起 sing-tun 自研栈为默认栈（`stack` 字段废弃），官方
 * build_libbox 已把 `with_gvisor` 移出 tags；本项目有意保留该 tag——mixed 栈
 * 的 UDP 半边与 desktop 遗留 `stack: "gvisor"` 序列化依赖 gVisor 编入。
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
