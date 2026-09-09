import { useCallback, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronDownIcon } from "@heroicons/react/24/outline";
import { Card, Chip, Spinner } from "@heroui/react";
import {
  CLOSED_CONNECTIONS_KEY,
  CONNECTIONS_KEY,
  connectionsActive,
  connectionsClosed,
  connectionsClose,
  toastError,
  toastSuccess,
  toErrorMessage,
  useProxyStatus,
} from "@pp/client-core";
import type { ActiveConnections, ConnectionView } from "@pp/client-core";
import { BackHeader } from "../components/BackHeader";
import { ConnectionCard } from "../components/ConnectionCard";

/** 轮询间隔（毫秒，与 desktop Connections 同频）。 */
const POLL_MS = 2000;
/** 出错后的降频间隔（毫秒）。 */
const POLL_ERROR_MS = 10000;

/** 居中提示卡（空态 / 加载中通用）。 */
function HintCard({ title, description }: { title: string; description: string }) {
  return (
    <Card>
      <Card.Content className="flex flex-col items-center justify-center gap-2 py-12 text-center">
        <span className="text-sm text-muted">{title}</span>
        <span className="text-xs text-muted/80">{description}</span>
      </Card.Content>
    </Card>
  );
}

/** 折叠区展开状态空态（占位小卡）。 */
function EmptyList({ text }: { text: string }) {
  return (
    <Card>
      <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
        <span className="text-sm text-muted">{text}</span>
      </Card.Content>
    </Card>
  );
}

/**
 * 连接页（ADR-0003 M5，路由 `/connections`，不在 TabBar）。
 *
 * - 数据：`connectionsActive` / `connectionsClosed` 每 2s 轮询（与首页流量卡相同启用
 *   条件：核心运行 && Clash API 就绪；出错降频 10s + `retry: false`，关闭单条时跳过轮询）；
 * - 展示：活跃连接单列卡片（host / 网络 chip / 代理链 / 命中规则 / ↑↓ 流量 / 时长），
 *   右上 × 单条关闭；「已关闭记录」为折叠区（同卡片结构，无关闭按钮）；
 * - 空态：核心未运行 → 「启动代理后可查看连接」；运行但 Clash API 未开 → 引导文案
 *   （对齐首页流量卡空态）。
 */
export default function Connections() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const running = status?.core_running ?? false;
  const clashApiUrl = status?.clash_api_url ?? null;
  const available = running && clashApiUrl !== null && clashApiUrl !== "";

  // 关闭动作期间跳过轮询，避免列表结果抖动（对齐 desktop）。
  const skipPollRef = useRef(false);
  const [closingId, setClosingId] = useState<string | null>(null);
  const [showClosed, setShowClosed] = useState(false);

  const { data: activeData, isLoading: activeLoading } = useQuery<ActiveConnections>({
    queryKey: CONNECTIONS_KEY,
    queryFn: connectionsActive,
    enabled: available,
    refetchInterval: (query) => {
      if (skipPollRef.current) return false;
      if (query.state.error) return POLL_ERROR_MS;
      return POLL_MS;
    },
    retry: false,
  });

  const { data: closedData, isLoading: closedLoading } = useQuery<ConnectionView[]>({
    queryKey: CLOSED_CONNECTIONS_KEY,
    queryFn: connectionsClosed,
    enabled: available,
    refetchInterval: (query) => {
      if (skipPollRef.current) return false;
      if (query.state.error) return POLL_ERROR_MS;
      return POLL_MS;
    },
    retry: false,
  });

  const activeConnections = activeData?.connections ?? [];
  const closedConnections = closedData ?? [];

  const handleClose = useCallback(
    async (id: string) => {
      setClosingId(id);
      skipPollRef.current = true;
      try {
        await connectionsClose(id);
        await queryClient.invalidateQueries({ queryKey: CONNECTIONS_KEY });
        toastSuccess("连接已关闭");
      } catch (err) {
        toastError(toErrorMessage(err));
      }
      setClosingId(null);
      window.setTimeout(() => {
        skipPollRef.current = false;
      }, 1500);
    },
    [queryClient],
  );

  return (
    <div className="flex min-h-full flex-col gap-4 pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="连接" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {/* 空态：核心未运行 / Clash API 未开启（对齐首页流量卡） */}
        {!available ? (
          !running ? (
            <HintCard title="启动代理后可查看连接" description="代理核心未运行，无法获取连接信息" />
          ) : (
            <HintCard title="在设置中开启 Clash API 后可查看连接" description="连接列表与流量统计均来自 Clash API" />
          )
        ) : activeLoading && activeConnections.length === 0 ? (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在获取连接…</span>
            </Card.Content>
          </Card>
        ) : (
          <>
            {/* 活跃连接列表 */}
            <section className="flex flex-col gap-2">
              <div className="flex items-center justify-between gap-2 px-1">
                <span className="min-w-0 truncate text-sm font-medium text-foreground">活跃连接</span>
                <Chip size="sm" variant="soft" color="default" className="shrink-0">
                  {activeConnections.length}
                </Chip>
              </div>
              {activeConnections.length === 0 ? (
                <HintCard title="暂无活跃连接" description="启动代理并产生流量后将显示连接" />
              ) : (
                <div className="flex flex-col gap-2">
                  {activeConnections.map((conn) => (
                    <ConnectionCard
                      key={conn.id}
                      conn={conn}
                      closing={closingId === conn.id}
                      onClose={(id) => void handleClose(id)}
                    />
                  ))}
                </div>
              )}
            </section>

            {/* 已关闭记录（折叠区） */}
            <section className="flex flex-col gap-2">
              <Card>
                <button
                  type="button"
                  onClick={() => setShowClosed((prev) => !prev)}
                  aria-expanded={showClosed}
                  className="flex w-full items-center gap-3 rounded-xl px-3 py-3 text-left active:opacity-80"
                >
                  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="truncate text-sm font-medium text-foreground">已关闭记录</span>
                    <span className="truncate text-xs text-muted">
                      {showClosed ? "点击收起已关闭的连接记录" : "连接关闭后将在此显示"}
                    </span>
                  </span>
                  <Chip size="sm" variant="soft" color="default" className="shrink-0">
                    {closedConnections.length}
                  </Chip>
                  <ChevronDownIcon
                    className={`size-5 shrink-0 text-muted transition-transform ${showClosed ? "rotate-180" : ""}`}
                    aria-hidden="true"
                  />
                </button>
              </Card>

              {showClosed &&
                (closedLoading && closedConnections.length === 0 ? (
                  <Card>
                    <Card.Content className="flex flex-col items-center justify-center gap-3 py-10 text-center">
                      <Spinner aria-hidden="true" />
                      <span className="text-sm text-muted">正在加载关闭记录…</span>
                    </Card.Content>
                  </Card>
                ) : closedConnections.length === 0 ? (
                  <EmptyList text="暂无已关闭记录" />
                ) : (
                  <div className="flex flex-col gap-2">
                    {closedConnections.map((conn) => (
                      <ConnectionCard key={conn.id} conn={conn} />
                    ))}
                  </div>
                ))}
            </section>
          </>
        )}
      </div>
    </div>
  );
}
