import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { Card, Chip } from "@pp/ui";
import { ChevronRightIcon } from "@heroicons/react/24/outline";
import { PROXIES_KEY, proxiesList, useClientConfig, useProxyStatus } from "@pp/client-core";
import type { ProxyList } from "@pp/client-core";
import { buildNodeMap, delayColor, delayText, mainProxyGroup } from "./proxyFormat";
import { RuleModeSwitch } from "./RuleModeSwitch";

/** 轮询间隔（毫秒，与 desktop Proxies 同频）。 */
const REFETCH_INTERVAL_MS = 5000;

interface ProxyStrategyCardProps {
  /** 核心是否运行（驱动查询 enabled 与空态文案）。 */
  running: boolean;
  /** 当前生效订阅名（`active_subscription_id` 匹配），为空则不展示该行。 */
  subscriptionName?: string | null;
}

/** 自动选择类分组类型（展示「自动选择」chip，区分手动/自动）。 */
function isAutoGroup(groupType: string): boolean {
  return groupType === "URLTest" || groupType === "Fallback";
}

/**
 * 双端首页代理策略卡片（ADR-0014）：模式切换与当前出口共用。
 *
 * - 数据：`proxiesList`（复用 `PROXIES_KEY` 轮询 5s，核心未运行时 enabled=false；
 *   出错降频 10s 自动重试）；订阅名由父级从 `SUBSCRIPTIONS_KEY` / `CONFIG_KEY` 复用传入；
 * - 展示：订阅名（小字 muted）+ 主出口分组（第一个 Selector 分组，无则兜底第一个分组）
 *   名称 + 当前 `now` 节点名 + 延迟 badge（分级对齐 desktop NodeItem）；
 *   `URLTest`/`Fallback` 分组标注「自动选择」chip；点击整卡进入共享 `/proxies` 策略页；
 * - 未运行时仍可设置下次启动使用的模式；无分组时保留模式与节点空态。
 */
export function ProxyStrategyCard({ running, subscriptionName }: ProxyStrategyCardProps) {
  const navigate = useNavigate();
  const { data: config } = useClientConfig();
  const { data: status } = useProxyStatus();
  const { data, isLoading, error } = useQuery<ProxyList>({
    queryKey: PROXIES_KEY,
    queryFn: proxiesList,
    enabled: running,
    refetchInterval: running ? (query) => (query.state.error ? REFETCH_INTERVAL_MS * 2 : REFETCH_INTERVAL_MS) : false,
    retry: false,
  });

  const group = running && data ? mainProxyGroup(data.groups) : null;
  const node = group ? buildNodeMap(data?.nodes ?? []).get(group.now) : null;
  const auto = group ? isAutoGroup(group.group_type) : false;

  return (
    <Card>
      <Card.Header>
        <Card.Title>代理策略</Card.Title>
        <Card.Description>选择流量路由模式与代理出口</Card.Description>
      </Card.Header>
      <Card.Content>
        <RuleModeSwitch
          value={status?.rule_mode ?? config?.rule_mode ?? "rule"}
          running={running}
          clashApiEnabled={config?.clash_api_enabled ?? false}
        />
      </Card.Content>
      <button
        type="button"
        onClick={() => navigate("/proxies")}
        aria-label="打开代理策略"
        className="flex min-h-12 w-full items-center gap-3 border-t border-border p-4 text-left hover:bg-surface-secondary active:opacity-80 focus-visible:outline-2 focus-visible:outline-focus"
      >
        {group ? (
          <>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              {subscriptionName && (
                <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{subscriptionName}</span>
              )}
              <span className="flex min-w-0 items-center gap-2">
                <span className="shrink-0 text-xs text-zinc-500 dark:text-zinc-400">{group.name}</span>
                <span className="truncate text-base font-semibold text-zinc-900 dark:text-zinc-100">{group.now}</span>
                {auto && (
                  <Chip color="default" className="shrink-0">
                    自动选择
                  </Chip>
                )}
              </span>
            </span>
            <Chip color={delayColor(node?.delay_ms)} className="shrink-0">
              {delayText(node?.delay_ms)}
            </Chip>
            <ChevronRightIcon className="size-5 shrink-0 text-zinc-400" aria-hidden="true" />
          </>
        ) : (
          <>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">代理节点</span>
              <span className="text-xs text-zinc-500 dark:text-zinc-400">
                {!running
                  ? "启动代理后可选择节点"
                  : isLoading
                    ? "正在获取节点…"
                    : error
                      ? "无法连接核心控制接口，点击查看"
                      : "暂无节点数据"}
              </span>
            </span>
            <ChevronRightIcon className="size-5 shrink-0 text-zinc-400" aria-hidden="true" />
          </>
        )}
      </button>
    </Card>
  );
}
