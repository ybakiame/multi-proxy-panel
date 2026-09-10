import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { Card, Chip } from "@heroui/react";
import { ChevronRightIcon } from "@heroicons/react/24/outline";
import { PROXIES_KEY, proxiesList } from "@pp/client-core";
import type { ProxyList } from "@pp/client-core";
import { buildNodeMap, delayColor, delayText, mainProxyGroup } from "./proxyFormat";

/** 轮询间隔（毫秒，与 desktop Proxies 同频）。 */
const REFETCH_INTERVAL_MS = 5000;

interface CurrentNodeCardProps {
  /** 核心是否运行（驱动查询 enabled 与空态文案）。 */
  running: boolean;
}

/**
 * 首页当前节点卡片（ADR-0003 M5）。
 *
 * - 数据：`proxiesList`（复用 `PROXIES_KEY` 轮询 5s，核心未运行时 enabled=false；
 *   出错降频 10s 自动重试）；
 * - 展示：主出口分组（第一个 Selector 分组，无则兜底第一个分组）名称 + 当前 `now` 节点名
 *   + 延迟 badge（分级对齐 desktop NodeItem）；点击整卡进入 `/proxies` 选择页；
 * - 空态：核心未运行 → "启动代理后可切换节点"（仍可点击进入选择页查看空态）；
 *   核心运行但无任何分组数据 → 隐藏卡片。
 */
export function CurrentNodeCard({ running }: CurrentNodeCardProps) {
  const navigate = useNavigate();
  const { data, isLoading } = useQuery<ProxyList>({
    queryKey: PROXIES_KEY,
    queryFn: proxiesList,
    enabled: running,
    refetchInterval: running ? (query) => (query.state.error ? REFETCH_INTERVAL_MS * 2 : REFETCH_INTERVAL_MS) : false,
    retry: false,
  });

  // 核心运行但已加载且无分组 → 隐藏卡片。
  if (running && data && data.groups.length === 0) {
    return null;
  }

  const group = data ? mainProxyGroup(data.groups) : null;
  const node = group ? buildNodeMap(data?.nodes ?? []).get(group.now) : null;

  return (
    <Card>
      <button
        type="button"
        onClick={() => navigate("/proxies")}
        aria-label="管理代理节点"
        className="flex min-h-12 w-full items-center gap-3 rounded-xl text-left active:opacity-80"
      >
        {group ? (
          <>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="truncate text-xs text-muted">{group.name}</span>
              <span className="truncate text-base font-semibold text-foreground">{group.now}</span>
            </span>
            <Chip size="sm" variant="soft" color={delayColor(node?.delay_ms)}>
              {delayText(node?.delay_ms)}
            </Chip>
            <ChevronRightIcon className="size-5 shrink-0 text-muted" aria-hidden="true" />
          </>
        ) : (
          <>
            <span className="flex min-w-0 flex-1 flex-col gap-0.5">
              <span className="text-sm font-medium text-foreground">代理节点</span>
              <span className="text-xs text-muted">
                {!running ? "启动代理后可切换节点" : isLoading ? "正在获取节点…" : "暂无节点数据"}
              </span>
            </span>
            <ChevronRightIcon className="size-5 shrink-0 text-muted" aria-hidden="true" />
          </>
        )}
      </button>
    </Card>
  );
}
