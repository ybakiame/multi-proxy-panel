/**
 * 内置常用 DNS 服务器预置目录（借鉴 karing `SettingConfigItemDNS.kDNSList`，按本端
 * DNS 切片 schema 裁剪：每条含 ISP 显示名、类型、地址与建议 tag）。
 *
 * 预置项在 DNS 管理「添加服务器」中以目录形式呈现，点选即物化为普通切片服务器
 * （`name` 取自 ISP 名，tag 经 `uniquePresetTag` 去重）；此后与手填服务器无差别，
 * 可启用 / 弃用 / 编辑 / 探测。目录按境内（低延迟直连）与境外（需经代理或用于
 * remote 语义）分组展示。
 */

import type { DnsServerType } from "./configSlices";

/** 单条预置 DNS 服务器。 */
export interface DnsServerPreset {
  /** ISP / 服务商显示名（写入切片 `name` 字段）。 */
  isp: string;
  serverType: DnsServerType;
  server: string;
  /** 缺省端口（与 sing-box 各类型默认一致）。 */
  serverPort: number | null;
  /** 建议 tag（冲突时由 `uniquePresetTag` 追加序号）。 */
  tag: string;
  /** 分组展示与行内提示（境内 / 境外）。 */
  group: "cn" | "global";
}

/** 内置预置目录（先境内后境外，同 ISP 内 udp → tls → https）。 */
export const DNS_SERVER_PRESETS: readonly DnsServerPreset[] = [
  // ---- 境内 ----
  { isp: "AliDNS", serverType: "udp", server: "223.5.5.5", serverPort: 53, tag: "alidns-udp", group: "cn" },
  { isp: "AliDNS", serverType: "udp", server: "223.6.6.6", serverPort: 53, tag: "alidns-udp-2", group: "cn" },
  { isp: "AliDNS", serverType: "tls", server: "223.5.5.5", serverPort: 853, tag: "alidns-tls", group: "cn" },
  {
    isp: "AliDNS",
    serverType: "https",
    server: "223.5.5.5",
    serverPort: 443,
    tag: "alidns-doh",
    group: "cn",
  },
  { isp: "DNSPod", serverType: "udp", server: "119.29.29.29", serverPort: 53, tag: "dnspod-udp", group: "cn" },
  { isp: "DNSPod", serverType: "tls", server: "dot.pub", serverPort: 853, tag: "dnspod-tls", group: "cn" },
  {
    isp: "DNSPod",
    serverType: "https",
    server: "doh.pub",
    serverPort: 443,
    tag: "dnspod-doh",
    group: "cn",
  },
  { isp: "114 DNS", serverType: "udp", server: "114.114.114.114", serverPort: 53, tag: "114dns-udp", group: "cn" },
  // ---- 境外 ----
  { isp: "Cloudflare", serverType: "udp", server: "1.1.1.1", serverPort: 53, tag: "cloudflare-udp", group: "global" },
  { isp: "Cloudflare", serverType: "tls", server: "1.1.1.1", serverPort: 853, tag: "cloudflare-tls", group: "global" },
  {
    isp: "Cloudflare",
    serverType: "https",
    server: "1.1.1.1",
    serverPort: 443,
    tag: "cloudflare-doh",
    group: "global",
  },
  { isp: "Google", serverType: "udp", server: "8.8.8.8", serverPort: 53, tag: "google-udp", group: "global" },
  { isp: "Google", serverType: "tls", server: "8.8.8.8", serverPort: 853, tag: "google-tls", group: "global" },
  { isp: "Google", serverType: "https", server: "8.8.8.8", serverPort: 443, tag: "google-doh", group: "global" },
  { isp: "AdGuard", serverType: "udp", server: "94.140.14.14", serverPort: 53, tag: "adguard-udp", group: "global" },
];

/**
 * 预置 tag 去重：与现有服务器 tag 冲突时追加 `-2` / `-3` …（对齐订阅节点 tag 去重语义）。
 */
export function uniquePresetTag(base: string, existing: readonly string[]): string {
  const used = new Set(existing.map((tag) => tag.trim()));
  if (!used.has(base)) {
    return base;
  }
  let n = 2;
  while (used.has(`${base}-${n}`)) {
    n += 1;
  }
  return `${base}-${n}`;
}
