import { useCallback, useMemo, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BoltIcon } from "@heroicons/react/24/outline";
import { Button, Card, Spinner } from "@heroui/react";
import {
  PROXIES_KEY,
  proxiesList,
  proxiesSelect,
  proxiesTestDelay,
  proxiesTestGroup,
  toastError,
  toastSuccess,
  toErrorMessage,
  useProxyStatus,
} from "@pp/client-core";
import type { GroupView, ProxyList } from "@pp/client-core";
import { BackHeader } from "../components/BackHeader";
import { ProxyNodeItem } from "../components/ProxyNodeItem";
import { buildNodeMap, groupTypeLabel, mainProxyGroup } from "../components/proxyFormat";

/** 轮询间隔（毫秒，与 desktop Proxies 同频）。 */
const REFETCH_INTERVAL_MS = 5000;

/** 居中提示卡（空态 / 加载等）。 */
function EmptyState({ title, description }: { title: string; description: string }) {
  return (
    <Card>
      <Card.Content className="flex flex-col items-center justify-center gap-2 py-12 text-center">
        <span className="text-sm text-muted">{title}</span>
        <span className="text-xs text-muted/80">{description}</span>
      </Card.Content>
    </Card>
  );
}

/**
 * 代理选择页（ADR-0003 M5，路由 `/proxies`，不在 TabBar）。
 *
 * - 返回头 + 顶部水平滚动分组 Chip（分组名 + 类型小字，当前分组高亮），右侧分组测速按钮；
 * - 节点列表：当前分组成员（名称 + 类型小字 + UDP 标 + 延迟 badge，选中行高亮）；
 *   仅 Selector 组可点击行切换，URLTest/Fallback/LoadBalance 由核心自动选择；
 * - 单节点测速：点节点行右侧延迟 badge；分组测速：顶部闪电按钮；
 * - 交互对齐 desktop Proxies：切换成功 toast + invalidate、测速结果 `setQueryData` 即时覆盖
 *   `delay_ms`、`skipPollRef` 跳过轮询 1.5s、错误降频 10s、`retry: false`；
 * - 空态：核心未运行 → "启动代理后可管理节点"。
 */
export default function Proxies() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const running = status?.core_running ?? false;

  // 测速/切换 busy 状态（Set 存名，避免并发重复触发）。
  const [testingGroups, setTestingGroups] = useState<Set<string>>(new Set());
  const [testingNodes, setTestingNodes] = useState<Set<string>>(new Set());
  const [selectBusy, setSelectBusy] = useState<Set<string>>(new Set());
  // 当前选中分组名；null = 跟随主出口分组（第一个 Selector，否则第一个分组）。
  const [activeGroupName, setActiveGroupName] = useState<string | null>(null);
  // 测速/切换期间跳过轮询，避免结果抖动（对齐 desktop）。
  const skipPollRef = useRef(false);

  const { data, isLoading, error } = useQuery<ProxyList>({
    queryKey: PROXIES_KEY,
    queryFn: proxiesList,
    enabled: running,
    refetchInterval: (query) => {
      if (skipPollRef.current) return false;
      // 出错（核心未就绪等）降频到 10s，避免刷屏。
      if (query.state.error) return REFETCH_INTERVAL_MS * 2;
      return REFETCH_INTERVAL_MS;
    },
    retry: false,
  });

  const groups = useMemo(() => data?.groups ?? [], [data]);
  const mainGroup = useMemo(() => mainProxyGroup(groups), [groups]);
  const activeGroup = useMemo(() => {
    if (activeGroupName) {
      const picked = groups.find((group) => group.name === activeGroupName);
      if (picked) return picked;
    }
    return mainGroup ?? null;
  }, [groups, activeGroupName, mainGroup]);
  const nodeMap = useMemo(() => buildNodeMap(data?.nodes ?? []), [data]);

  /** 切换节点（仅 Selector 组可触发，入口已由 ProxyNodeItem 的 selectable 控制）。 */
  const handleSelect = useCallback(
    async (group: string, name: string) => {
      setSelectBusy((prev) => new Set(prev).add(`${group}:${name}`));
      skipPollRef.current = true;
      try {
        await proxiesSelect(group, name);
        await queryClient.invalidateQueries({ queryKey: PROXIES_KEY });
        toastSuccess(`已切换至「${name}」`);
      } catch (err) {
        toastError(toErrorMessage(err));
      }
      setSelectBusy((prev) => {
        const next = new Set(prev);
        next.delete(`${group}:${name}`);
        return next;
      });
      // 延迟恢复轮询，让 invalidate 先完成。
      window.setTimeout(() => {
        skipPollRef.current = false;
      }, 1500);
    },
    [queryClient],
  );

  /** 单节点测速。 */
  const handleTestNode = useCallback(
    async (name: string) => {
      setTestingNodes((prev) => new Set(prev).add(name));
      skipPollRef.current = true;
      try {
        const delay = await proxiesTestDelay(name);
        // 即时刷新本地数据：覆盖对应节点的 delay_ms（对齐 desktop）。
        queryClient.setQueryData<ProxyList>(PROXIES_KEY, (old) => {
          if (!old) return old;
          return {
            ...old,
            nodes: old.nodes.map((node) => (node.name === name ? { ...node, delay_ms: delay } : node)),
          };
        });
      } catch (err) {
        toastError(toErrorMessage(err));
      }
      setTestingNodes((prev) => {
        const next = new Set(prev);
        next.delete(name);
        return next;
      });
      window.setTimeout(() => {
        skipPollRef.current = false;
      }, 1500);
    },
    [queryClient],
  );

  /** 全组测速（顶部闪电按钮）。 */
  const handleTestGroup = useCallback(
    async (group: GroupView) => {
      setTestingGroups((prev) => new Set(prev).add(group.name));
      skipPollRef.current = true;
      try {
        const results = await proxiesTestGroup(group.name);
        const delayMap = new Map<string, number | null>();
        for (const result of results) {
          delayMap.set(result.name, result.delay_ms);
        }
        queryClient.setQueryData<ProxyList>(PROXIES_KEY, (old) => {
          if (!old) return old;
          return {
            ...old,
            nodes: old.nodes.map((node) =>
              delayMap.has(node.name) ? { ...node, delay_ms: delayMap.get(node.name) ?? null } : node,
            ),
          };
        });
        const okCount = results.filter((result) => result.delay_ms != null).length;
        toastSuccess(`测速完成：${okCount}/${results.length} 个节点可用`);
      } catch (err) {
        toastError(toErrorMessage(err));
      }
      setTestingGroups((prev) => {
        const next = new Set(prev);
        next.delete(group.name);
        return next;
      });
      window.setTimeout(() => {
        skipPollRef.current = false;
      }, 1500);
    },
    [queryClient],
  );

  const isSelector = activeGroup?.group_type === "Selector";
  const groupTesting = activeGroup !== null && testingGroups.has(activeGroup.name);

  return (
    <div className="flex min-h-full flex-col gap-4 pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="代理选择" />
      <div
        className="flex flex-col gap-4"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {!running ? (
          <EmptyState title="启动代理后可管理节点" description="代理核心未运行，无法获取节点列表与测速" />
        ) : isLoading ? (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在获取节点列表…</span>
            </Card.Content>
          </Card>
        ) : error ? (
          <EmptyState title="节点列表获取失败" description="请确认代理核心已就绪，将自动重试" />
        ) : !activeGroup ? (
          <EmptyState title="暂无代理分组" description="请检查订阅配置或核心配置是否包含代理分组" />
        ) : (
          <>
            {/* 分组切换：水平滚动 Chip + 右侧分组测速 */}
            <div className="flex items-center gap-2">
              <div className="min-w-0 flex-1 overflow-x-auto py-0.5">
                <div className="flex w-max items-center gap-2 pr-1">
                  {groups.map((group) => {
                    const isActive = group.name === activeGroup.name;
                    return (
                      <button
                        key={group.name}
                        type="button"
                        onClick={() => setActiveGroupName(group.name)}
                        aria-pressed={isActive}
                        className={`flex shrink-0 flex-col items-center gap-0 rounded-full border px-4 py-1.5 transition-colors ${
                          isActive ? "border-primary/60 bg-primary/10" : "border-border/60 bg-surface active:opacity-70"
                        }`}
                      >
                        <span
                          className={`text-sm font-medium leading-tight ${isActive ? "text-primary" : "text-foreground"}`}
                        >
                          {group.name}
                        </span>
                        <span className={`text-[10px] leading-tight ${isActive ? "text-primary/80" : "text-muted"}`}>
                          {groupTypeLabel(group.group_type)}
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>
              <Button
                variant="secondary"
                isIconOnly
                aria-label={`对分组 ${activeGroup.name} 测速`}
                isDisabled={groupTesting}
                isPending={groupTesting}
                onPress={() => void handleTestGroup(activeGroup)}
                className="shrink-0"
              >
                {!groupTesting && <BoltIcon className="size-5" aria-hidden="true" />}
              </Button>
            </div>

            {!isSelector && (
              <p className="px-1 text-xs text-muted">
                {groupTypeLabel(activeGroup.group_type)}组由核心自动选择，点击节点行可单独测速
              </p>
            )}

            {/* 节点列表 */}
            <div className="flex flex-col gap-2">
              <p className="px-1 text-xs text-muted">
                当前：<span className="font-medium text-foreground">{activeGroup.now}</span>
              </p>
              {activeGroup.members.map((member) => {
                const node = nodeMap.get(member);
                const selected = activeGroup.now === member;
                const busy = selectBusy.has(`${activeGroup.name}:${member}`);
                const testing = testingNodes.has(member);
                return (
                  <ProxyNodeItem
                    key={member}
                    name={member}
                    node={node}
                    selected={selected}
                    selectable={isSelector}
                    busy={busy}
                    testing={testing}
                    onSelect={() => void handleSelect(activeGroup.name, member)}
                    onTest={() => void handleTestNode(member)}
                  />
                );
              })}
              {activeGroup.members.length === 0 && (
                <EmptyState title="该分组暂无节点" description="请检查订阅配置或核心配置" />
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
