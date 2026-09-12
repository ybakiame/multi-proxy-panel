/**
 * 内置 CN 分流基线只读视图 API。
 *
 * Aligned with Rust-side `BaselineView`
 * (crates/pp-client/src/core_config/view.rs)。纯静态、无参、不读盘：
 * 前端把基线与各配置列表只读合并展示，所有项不可编辑 / 删除。
 */

import { invoke } from "@tauri-apps/api/core";

/** 内置规则集视图项（只读）。 */
export interface BaselineRuleSetView {
  /** 规则集 tag（如 `geosite-cn`）。 */
  tag: string;
  /** 远程规则集 URL。 */
  url: string;
  /** 中文友好名（如「国内域名」）。 */
  description: string;
}

/** 内置路由规则视图项（只读）。 */
export interface BaselineRuleView {
  /** 中文描述（如「国内域名 → 直连」）。 */
  description: string;
  /** 命中的规则集 tag 列表。 */
  rule_set_tags: string[];
  /** 命中后的出站 tag。 */
  outbound: string;
}

/** 内置 DNS 规则视图项（只读；本次不在 DNS 页展示，保留类型完整性）。 */
export interface BaselineDnsRuleView {
  /** 中文描述（如「直连模式：DNS 走本地解析器」）。 */
  description: string;
  /** 分流到的 DNS server tag（`local` / `remote`）。 */
  server: string;
  /** clash_mode 条件（`direct` / `global`）；非模式规则缺省。 */
  clash_mode?: string;
  /** 命中的规则集 tag 列表；模式规则缺省。 */
  rule_set_tags?: string[];
}

/** 内置出站视图项（只读）。 */
export interface BaselineOutboundView {
  /** 出站 tag（`proxy` / `auto` / `direct` / `block`）。 */
  tag: string;
  /** 出站 kind（sing-box `type`）。 */
  kind: string;
  /** 中文描述（如「手动选择」）。 */
  description: string;
}

/** 内置 CN 分流基线只读视图。 */
export interface BaselineView {
  /** 5 个 CN 规则集。 */
  rule_sets: BaselineRuleSetView[];
  /** 5 条基线路由规则。 */
  route_rules: BaselineRuleView[];
  /** 基线 DNS 规则摘要。 */
  dns_rules: BaselineDnsRuleView[];
  /** 内置出站：proxy / auto / direct / block。 */
  outbounds: BaselineOutboundView[];
  /** `route.final`（主选择器 tag）。 */
  route_final: string;
}

/** 拉取内置 CN 分流基线只读视图（无参、纯静态、不读盘）。 */
export function baselineViewGet(): Promise<BaselineView> {
  return invoke<BaselineView>("baseline_view_get");
}
