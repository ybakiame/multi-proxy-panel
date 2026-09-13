import type { ConfigSlices, NodeTagView, RouteSlice } from "@pp/client-core";
import { outboundTag } from "@pp/client-core";
import type { MobileSelectOption } from "../../../components/MobileSelectSheet";

/**
 * 路由切片（ADR-0005）纯逻辑：候选构建 / 校验。
 *
 * 与页面组件分离，便于单测与复用；候选并集对齐 CustomRulesPage 的「指定出站」
 * 模式（内置 + 切片 + 静态订阅节点，按 tag 去重），校验对齐 Rust
 * `RouteSlice::validate`：非空的 `final_tag` / `resolver.server`
 * 不得包含空白字符（引用完整性交给核心运行时校验）。
 */

/** 内置出站 tag（模板层固定，始终可作 `route.final` 目标）。 */
export const BUILTIN_OUTBOUND_TAGS = ["proxy", "auto", "direct", "block"] as const;

/** 内置 DNS server tag（模板层固定：local = 系统 DNS，remote = 代理 DoH）。 */
export const BUILTIN_DNS_SERVER_TAGS = ["local", "remote"] as const;

/** 内置出站候选展示说明。 */
const BUILTIN_OUTBOUND_HINT = "内置出站";

/** 内置 DNS server 候选展示说明。 */
const BUILTIN_DNS_SERVER_HINT = "内置 DNS";

/** `route.final` 默认（置空 = 模板默认 `proxy`）选项。 */
export const DEFAULT_FINAL_OPTION: MobileSelectOption = { value: "", label: "默认（proxy）" };

/** `default_domain_resolver.server` 默认（置空 = 不覆写）选项。 */
export const DEFAULT_RESOLVER_SERVER_OPTION: MobileSelectOption = { value: "", label: "默认" };

/** 路由解析策略候选：空串 = 跟随全局 DNS 策略（不渲染 `strategy`）。 */
export const ROUTE_STRATEGY_OPTIONS: MobileSelectOption[] = [
  { value: "", label: "跟随全局" },
  { value: "prefer_ipv4", label: "优先 IPv4" },
  { value: "prefer_ipv6", label: "优先 IPv6" },
  { value: "ipv4_only", label: "仅 IPv4" },
  { value: "ipv6_only", label: "仅 IPv6" },
];

/**
 * 构建 `route.final` 候选并集：内置出站 ∪ 启用中的切片出站（含分组）∪ 静态订阅节点。
 *
 * - 内置出站：模板固定 `proxy` / `auto` / `direct` / `block`；
 * - 切片出站：tag 由名称生成（`outboundTag`），仅列启用项；
 * - 订阅节点：`subscription_node_tags`（生效订阅本地缓存，静态源，不依赖核心运行）。
 */
export function buildFinalTagOptions(
  slices: ConfigSlices | null,
  subscriptionNodes: NodeTagView[] | undefined,
): MobileSelectOption[] {
  const options: MobileSelectOption[] = [DEFAULT_FINAL_OPTION];
  const seen = new Set<string>();
  const push = (value: string, label: string, description?: string) => {
    const tag = value.trim();
    if (tag === "" || seen.has(tag)) return;
    seen.add(tag);
    options.push({ value: tag, label, description });
  };

  for (const tag of BUILTIN_OUTBOUND_TAGS) push(tag, tag, BUILTIN_OUTBOUND_HINT);
  for (const item of slices?.outbounds.items ?? []) {
    if (!item.enabled) continue;
    const tag = outboundTag(item.name);
    push(tag, item.name.trim() || tag, tag);
  }
  for (const node of subscriptionNodes ?? []) push(node.tag, node.name.trim() || node.tag, "订阅节点");

  return options;
}

/**
 * 构建 `default_domain_resolver.server` 候选并集：内置 DNS server ∪ DNS 切片 server。
 *
 * - 内置：`local` / `remote`；`dns_fakeip_enabled` 时追加 `fakeip`；
 * - DNS 切片：仅当切片 mode 为 `takeover` 时其 server tag 才会被注入，故仅此时列出。
 */
export function buildResolverServerOptions(slices: ConfigSlices | null, fakeipEnabled: boolean): MobileSelectOption[] {
  const options: MobileSelectOption[] = [DEFAULT_RESOLVER_SERVER_OPTION];
  const seen = new Set<string>();
  const push = (value: string, label: string, description?: string) => {
    const tag = value.trim();
    if (tag === "" || seen.has(tag)) return;
    seen.add(tag);
    options.push({ value: tag, label, description });
  };

  for (const tag of BUILTIN_DNS_SERVER_TAGS) push(tag, tag, BUILTIN_DNS_SERVER_HINT);
  if (fakeipEnabled) push("fakeip", "fakeip", BUILTIN_DNS_SERVER_HINT);

  const dns = slices?.dns;
  if (dns?.mode === "takeover") {
    // 弃用（enabled=false）的切片服务器不渲染进运行配置，不可选。
    for (const server of dns.servers) {
      if (server.enabled) push(server.tag, server.tag, "DNS 切片服务器");
    }
  }

  return options;
}

/** 路由切片字段错误（`null` = 合法）。 */
export interface RouteSliceErrors {
  finalTag: string | null;
  resolverServer: string | null;
}

/** 校验 route 切片草稿（保存前调用）。 */
export function validateRouteSlice(route: RouteSlice): RouteSliceErrors {
  return {
    finalTag: route.final_tag !== "" && /\s/.test(route.final_tag) ? "不能包含空白字符" : null,
    resolverServer: route.resolver.server !== "" && /\s/.test(route.resolver.server) ? "不能包含空白字符" : null,
  };
}

/** 校验结果是否全部通过。 */
export function isRouteSliceValid(errors: RouteSliceErrors): boolean {
  return errors.finalTag === null && errors.resolverServer === null;
}
