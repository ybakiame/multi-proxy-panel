/**
 * 内置基线只读视图 API。
 *
 * Aligned with Rust-side `BaselineView`
 * (crates/pp-client/src/core_config/view.rs)。纯静态、无参、不读盘。
 *
 * 规则集 / 路由规则已物化为用户可编辑的条目，本视图退化为**还原模板**
 * （「重置内置规则集」/「恢复内置规则」按 `id` 重建条目）；出站仍为只读。
 */

import { invoke } from "@tauri-apps/api/core";

/** 内置规则集还原模板（私有域名 / 私有 IP）。 */
export interface BaselineRuleSetView {
  /** 稳定条目 ID（物化进 `custom_rule_sets` 的 `id`）。 */
  id: string;
  /** 规则集 tag（如 `geosite-private`）。 */
  tag: string;
  /** 远程规则集 URL。 */
  url: string;
  /** 展示名（如「私有域名」）。 */
  name: string;
}

/** 内置路由规则还原模板（私有域名 + 私有 IP → 直连）。 */
export interface BaselineRuleView {
  /** 稳定规则 ID（物化进统一规则列表的 `id`）。 */
  id: string;
  /** 规则名称（如「内置：私有域名与私有 IP 直连」）。 */
  name: string;
  /** 中文描述（如「私有域名、私有 IP → 直连」）。 */
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

/** 内置基线只读视图。 */
export interface BaselineView {
  /** 内置规则集还原模板（私有域名 / 私有 IP）。 */
  rule_sets: BaselineRuleSetView[];
  /** 内置路由规则还原模板（私有域名 + 私有 IP → 直连）。 */
  route_rules: BaselineRuleView[];
  /** 基线 DNS 规则摘要。 */
  dns_rules: BaselineDnsRuleView[];
  /** 内置出站：proxy / auto / direct / block。 */
  outbounds: BaselineOutboundView[];
  /** `route.final`（主选择器 tag）。 */
  route_final: string;
}

/** 拉取内置基线只读视图（无参、纯静态、不读盘）。 */
export function baselineViewGet(): Promise<BaselineView> {
  return invoke<BaselineView>("baseline_view_get");
}
