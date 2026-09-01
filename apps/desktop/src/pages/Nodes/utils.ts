import type { SubscriptionUserInfo, SubscriptionFormat } from "../../api";

/** Format bytes to GB (2 decimal places). */
export function formatGb(bytes: number | null | undefined): string {
  if (bytes == null) {
    return "-";
  }
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

/** Used traffic = download + upload; when total is missing only show used. */
export function usageText(info: SubscriptionUserInfo | null): string {
  if (!info) {
    return "-";
  }
  const used = (info.download ?? 0) + (info.upload ?? 0);
  if (info.total == null) {
    return formatGb(used);
  }
  return `${formatGb(used)} / ${formatGb(info.total)}`;
}

/** Usage percentage (0-100, 0 when total is missing). */
export function usagePercent(info: SubscriptionUserInfo | null): number {
  if (!info || info.total == null || info.total <= 0) {
    return 0;
  }
  const used = (info.download ?? 0) + (info.upload ?? 0);
  return Math.min(100, Math.max(0, (used / info.total) * 100));
}

/** Expiration timestamp (seconds) to local date; missing returns placeholder. */
export function formatExpire(expire: number | null | undefined): string {
  if (expire == null) {
    return "-";
  }
  const date = new Date(expire * 1000);
  return Number.isNaN(date.getTime()) ? "-" : date.toLocaleDateString();
}

/** Usage progress bar color: over-quota red, >80% yellow, else accent. */
export function usageColor(percent: number): "accent" | "warning" | "danger" {
  if (percent >= 100) {
    return "danger";
  }
  if (percent >= 80) {
    return "warning";
  }
  return "accent";
}

/** Common UA quick selections (empty = default clash.meta). */
export const UA_PRESETS = [
  { value: "", label: "默认" },
  { value: "clash.meta", label: "clash.meta" },
  { value: "clash-verge", label: "clash-verge" },
  { value: "sing-box", label: "sing-box" },
];

/** Subscription format display name: ClashYaml → Clash, SingBoxJson → sing-box. */
export function formatLabel(format: SubscriptionFormat): string {
  if (format === "ClashYaml") {
    return "Clash";
  }
  if (format === "SingBoxJson") {
    return "sing-box";
  }
  return "ShareLinks";
}

/** Subscription format Chip color: ShareLinks accent, ClashYaml warning, SingBoxJson success. */
export function formatColor(format: SubscriptionFormat): "accent" | "warning" | "success" {
  if (format === "ClashYaml") {
    return "warning";
  }
  if (format === "SingBoxJson") {
    return "success";
  }
  return "accent";
}

/** Core type display name: singbox → sing-box, mihomo → mihomo (for override association hint). */
export function coreLabel(coreType: string | undefined): string {
  return coreType === "mihomo" ? "mihomo" : "sing-box";
}

/** Derive compatible core from subscription format; ShareLinks/empty returns null (follows global core). */
export function subCoreType(format: SubscriptionFormat | null | undefined): "singbox" | "mihomo" | null {
  if (format === "ClashYaml") {
    return "mihomo";
  }
  if (format === "SingBoxJson") {
    return "singbox";
  }
  return null;
}
