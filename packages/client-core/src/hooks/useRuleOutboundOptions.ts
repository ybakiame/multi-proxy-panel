import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { configSlicesGet, proxiesList, subscriptionNodeTags } from "../api";
import type { ConfigSlices, NodeTagView, ProxyList } from "../api";
import { CONFIG_SLICES_KEY, PROXIES_KEY, subscriptionNodeTagsKey } from "../api/keys";
import { buildRuleOutboundOptions } from "../ruleOutboundOptions";
import type { RuleOutboundOption } from "../ruleOutboundOptions";
import { useClientConfig } from "./useClientConfig";
import { useProxyStatus } from "./useProxyStatus";

export interface RuleOutboundOptionsResult {
  /** 指定出站候选：静态订阅节点 + 模板分组 + 切片出站并集（按 tag 去重）。 */
  options: RuleOutboundOption[];
  /** 生效订阅存在且缓存非空：候选为空时据此区分「先同步订阅」与「先添加切片出站」引导。 */
  subscriptionCacheAvailable: boolean;
}

/**
 * 「指定出站」动作候选的数据装配（ADR-0011 D1：双端同构视图模型下沉 client-core）。
 *
 * 数据源（复用各页同 key 缓存，不在消费组件内新起重型查询）：
 * - 切片出站读 `config_slices`（CONFIG_SLICES_KEY）；
 * - 订阅节点读生效订阅的本地缓存（静态源，不依赖核心运行）；
 * - 模板分组读运行中核心的 `proxies_list`（PROXIES_KEY，与首页/代理页共享缓存，
 *   核心未运行时不发起）。
 */
export function useRuleOutboundOptions(): RuleOutboundOptionsResult {
  const { data: status } = useProxyStatus();
  const coreRunning = status?.core_running ?? false;

  const { data: slices } = useQuery<ConfigSlices>({
    queryKey: CONFIG_SLICES_KEY,
    queryFn: configSlicesGet,
  });
  const { data: config } = useClientConfig();
  const activeSubscriptionId = config?.active_subscription_id ?? null;
  const { data: subscriptionNodes } = useQuery<NodeTagView[]>({
    queryKey: subscriptionNodeTagsKey(activeSubscriptionId ?? ""),
    queryFn: () => subscriptionNodeTags(activeSubscriptionId ?? ""),
    enabled: !!activeSubscriptionId,
    retry: false,
  });
  const { data: proxyList } = useQuery<ProxyList>({
    queryKey: PROXIES_KEY,
    queryFn: proxiesList,
    enabled: coreRunning,
    retry: false,
  });

  // 生效订阅存在但缓存为空（从未同步 / 缓存丢失）：给「先同步订阅」引导文案。
  const subscriptionCacheAvailable = !!activeSubscriptionId && (subscriptionNodes?.length ?? 0) > 0;

  const options = useMemo(
    () => buildRuleOutboundOptions(subscriptionNodes, proxyList?.groups, slices),
    [subscriptionNodes, proxyList, slices],
  );

  return { options, subscriptionCacheAvailable };
}
