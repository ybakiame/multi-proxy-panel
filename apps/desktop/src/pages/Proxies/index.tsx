import { useCallback, useRef, useState } from "react";
import { Alert, Button, Card, Chip, Spinner } from "@heroui/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { proxiesList, proxiesSelect, proxiesTestDelay, proxiesTestGroup, toErrorMessage } from "../../api";
import type { GroupView, NodeView, ProxyList } from "../../api";
import { toastError, toastSuccess } from "../../toast";
import { useAppStore } from "../../store";
import NodeItem from "./NodeItem";

const PROXIES_KEY = ["proxies_list"];
const REFETCH_INTERVAL_MS = 5000;

/** 分组类型中文标签。 */
function groupTypeLabel(type: string): string {
  const map: Record<string, string> = {
    Selector: "选择器",
    URLTest: "自动测速",
    Fallback: "故障转移",
    LoadBalance: "负载均衡",
  };
  return map[type] ?? type;
}

/** 构建节点名 → 节点详情的映射表。 */
function buildNodeMap(nodes: NodeView[]): Map<string, NodeView> {
  const map = new Map<string, NodeView>();
  for (const node of nodes) {
    map.set(node.name, node);
  }
  return map;
}

export default function Proxies() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const status = useAppStore((s) => s.status);
  const _running = status?.core_running ?? false;

  // 测速中状态：分组名 → boolean；节点名 → boolean
  const [testingGroups, setTestingGroups] = useState<Set<string>>(new Set());
  const [testingNodes, setTestingNodes] = useState<Set<string>>(new Set());
  const [selectBusy, setSelectBusy] = useState<Set<string>>(new Set());
  const [actionError, setActionError] = useState<string | null>(null);

  // 用于在测速/切换期间跳过轮询，避免抖动
  const skipPollRef = useRef(false);

  const { data, isLoading, error } = useQuery<ProxyList>({
    queryKey: PROXIES_KEY,
    queryFn: proxiesList,
    refetchInterval: (query) => {
      if (skipPollRef.current) return false;
      // 若上次查询出错（核心未运行），降低轮询频率到 10s，避免刷屏
      if (query.state.error) return 10000;
      return REFETCH_INTERVAL_MS;
    },
    retry: false,
  });

  // 核心未运行时 error 不为 null
  const coreNotRunning = Boolean(error);

  const nodeMap = data ? buildNodeMap(data.nodes) : new Map<string, NodeView>();

  /** 切换节点（仅 Selector 类型组可点）。 */
  const handleSelect = useCallback(
    async (group: string, name: string) => {
      setSelectBusy((prev) => new Set(prev).add(`${group}:${name}`));
      skipPollRef.current = true;
      setActionError(null);
      try {
        await proxiesSelect(group, name);
        await queryClient.invalidateQueries({ queryKey: PROXIES_KEY });
        toastSuccess(`已切换至「${name}」`);
      } catch (err) {
        const msg = toErrorMessage(err);
        setActionError(msg);
        toastError(msg);
      } finally {
        setSelectBusy((prev) => {
          const next = new Set(prev);
          next.delete(`${group}:${name}`);
          return next;
        });
        // 延迟恢复轮询，让 invalidate 先完成
        window.setTimeout(() => {
          skipPollRef.current = false;
        }, 1500);
      }
    },
    [queryClient],
  );

  /** 单节点测速。 */
  const handleTestNode = useCallback(
    async (name: string) => {
      setTestingNodes((prev) => new Set(prev).add(name));
      skipPollRef.current = true;
      setActionError(null);
      try {
        const delay = await proxiesTestDelay(name);
        // 即时刷新本地数据：覆盖对应节点的 delay_ms
        queryClient.setQueryData<ProxyList>(PROXIES_KEY, (old) => {
          if (!old) return old;
          return {
            ...old,
            nodes: old.nodes.map((n) => (n.name === name ? { ...n, delay_ms: delay } : n)),
          };
        });
      } catch (err) {
        const msg = toErrorMessage(err);
        setActionError(msg);
        toastError(msg);
      } finally {
        setTestingNodes((prev) => {
          const next = new Set(prev);
          next.delete(name);
          return next;
        });
        window.setTimeout(() => {
          skipPollRef.current = false;
        }, 1500);
      }
    },
    [queryClient],
  );

  /** 全组测速。 */
  const handleTestGroup = useCallback(
    async (group: GroupView) => {
      setTestingGroups((prev) => new Set(prev).add(group.name));
      skipPollRef.current = true;
      setActionError(null);
      try {
        const results = await proxiesTestGroup(group.name);
        const delayMap = new Map<string, number | null>();
        for (const r of results) {
          delayMap.set(r.name, r.delay_ms);
        }
        // 即时刷新本地数据
        queryClient.setQueryData<ProxyList>(PROXIES_KEY, (old) => {
          if (!old) return old;
          return {
            ...old,
            nodes: old.nodes.map((n) => {
              if (delayMap.has(n.name)) {
                return { ...n, delay_ms: delayMap.get(n.name) ?? null };
              }
              return n;
            }),
          };
        });
        const okCount = results.filter((r) => r.delay_ms != null).length;
        toastSuccess(`测速完成：${okCount}/${results.length} 个节点可用`);
      } catch (err) {
        const msg = toErrorMessage(err);
        setActionError(msg);
        toastError(msg);
      } finally {
        setTestingGroups((prev) => {
          const next = new Set(prev);
          next.delete(group.name);
          return next;
        });
        window.setTimeout(() => {
          skipPollRef.current = false;
        }, 1500);
      }
    },
    [queryClient],
  );

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">策略组</h1>
          <p className="text-sm text-muted">节点分组、选择与延迟测试</p>
        </div>
        <Button variant="secondary" size="sm" onPress={() => navigate("/nodes")}>
          管理订阅
        </Button>
      </div>

      {actionError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description className="break-all">{actionError}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {coreNotRunning && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-4 py-12 text-center">
            <span className="text-sm text-muted">启动代理后可用</span>
            <span className="text-xs text-muted/80">代理核心未运行，无法获取节点列表与测速</span>
            <Button variant="primary" onPress={() => navigate("/")}>
              前往首页
            </Button>
          </Card.Content>
        </Card>
      )}

      {isLoading && !coreNotRunning && (
        <div className="flex items-center justify-center py-12">
          <Spinner />
        </div>
      )}

      {!coreNotRunning && data && (
        <div className="flex flex-col gap-4">
          {data.groups.map((group) => {
            const isSelector = group.group_type === "Selector";
            const isUrlTest = group.group_type === "URLTest";
            const groupTesting = testingGroups.has(group.name);

            return (
              <Card key={group.name}>
                <Card.Header className="flex flex-wrap items-center justify-between gap-2">
                  <div className="flex items-center gap-2">
                    <Card.Title>{group.name}</Card.Title>
                    <Chip size="sm" variant="soft" color="default">
                      {groupTypeLabel(group.group_type)}
                    </Chip>
                  </div>
                  <div className="flex items-center gap-2">
                    <span className="text-xs text-muted">
                      当前：
                      <span className="font-medium text-foreground">{group.now}</span>
                    </span>
                    <Button
                      size="sm"
                      variant="secondary"
                      isPending={groupTesting}
                      isDisabled={groupTesting}
                      onPress={() => void handleTestGroup(group)}
                    >
                      {groupTesting ? "测速中" : "测速"}
                    </Button>
                  </div>
                </Card.Header>
                <Card.Content>
                  <div className="grid grid-cols-1 gap-2 sm:grid-cols-2 lg:grid-cols-3">
                    {group.members.map((memberName) => {
                      const node = nodeMap.get(memberName);
                      const selected = group.now === memberName;
                      const busy = selectBusy.has(`${group.name}:${memberName}`);
                      const nodeTesting = testingNodes.has(memberName);

                      return (
                        <NodeItem
                          key={memberName}
                          name={memberName}
                          node={node}
                          selected={selected}
                          selectable={isSelector}
                          isAuto={isUrlTest}
                          busy={busy}
                          testing={nodeTesting}
                          onSelect={() => void handleSelect(group.name, memberName)}
                          onTest={() => void handleTestNode(memberName)}
                        />
                      );
                    })}
                  </div>
                </Card.Content>
              </Card>
            );
          })}

          {data.groups.length === 0 && (
            <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
              <span className="text-sm text-muted">暂无代理分组</span>
              <span className="text-xs text-muted/80">请检查订阅配置或核心配置是否包含代理分组</span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
