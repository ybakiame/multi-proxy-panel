import type { ConfigSlices, GroupView, NodeTagView } from "./api";
import { outboundTag } from "./api";

/**
 * 「指定出站」动作的出站候选（路由规则表单，ADR-0005 §3.1）。
 *
 * 候选 = 静态订阅节点 ∪ 运行中模板分组 ∪ 启用中的切片出站，按 tag 去重。
 * 数据装配由 `hooks/useRuleOutboundOptions` 统一完成，双端页面只消费本模块
 * 的构建结果，禁止各页另造并集逻辑（ADR-0011 D1/D3）。
 */
export interface RuleOutboundOption {
  /** 写入 `outbound:<tag>` 的 tag。 */
  value: string;
  /** 下拉显示名（切片出站用名称，订阅节点/模板分组用 tag）。 */
  label: string;
  /** 可选说明行（切片 tag / 来源标注）。 */
  hint?: string;
}

/**
 * 构建「指定出站」候选 tag 并集：
 *
 * - 静态订阅节点：`subscription_node_tags`（生效订阅的本地缓存），不依赖核心运行；
 * - 模板分组：`proxiesList.groups`（经 Clash API 读取运行中核心），核心未运行时
 *   无数据；订阅节点不在其中（已由静态源覆盖）；
 * - 切片出站：tag 由名称生成（`outboundTag`），仅列启用项（父切片总开关由注入层判定）。
 */
export function buildRuleOutboundOptions(
  subscriptionNodes: readonly NodeTagView[] | null | undefined,
  proxyGroups: readonly GroupView[] | null | undefined,
  slices: ConfigSlices | null | undefined,
): RuleOutboundOption[] {
  const options: RuleOutboundOption[] = [];
  const seen = new Set<string>();
  const push = (value: string, label: string, hint?: string) => {
    const tag = value.trim();
    if (tag === "" || seen.has(tag)) return;
    seen.add(tag);
    options.push({ value: tag, label, hint });
  };
  for (const node of subscriptionNodes ?? []) push(node.tag, node.name.trim() || node.tag, "订阅节点");
  for (const group of proxyGroups ?? []) push(group.name, group.name, "模板分组");
  for (const item of slices?.outbounds.items ?? []) {
    if (!item.enabled) continue;
    const tag = outboundTag(item.name);
    push(tag, item.name, tag);
  }
  return options;
}
