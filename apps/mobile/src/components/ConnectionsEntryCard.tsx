import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { ArrowsRightLeftIcon } from "@heroicons/react/24/outline";
import { CONNECTIONS_KEY, connectionsActive } from "@pp/client-core";
import type { ActiveConnections } from "@pp/client-core";
import { EntryLinkCard } from "./EntryLinkCard";

/** 轮询间隔（毫秒，与 TrafficCard / desktop Connections 同频）。 */
const POLL_MS = 2000;

interface ConnectionsEntryCardProps {
  /** 核心是否运行。 */
  running: boolean;
  /** 运行中且 Clash API 开启时的面板地址；核心未运行或 API 关闭时为 `null`。 */
  clashApiUrl: string | null;
}

/**
 * 首页「当前连接」入口卡（流量卡下方，点击进 `/connections`）。
 *
 * 与 TrafficCard 共用 `CONNECTIONS_KEY`（同 key 同选项，观察者合并、不重复请求），
 * 描述行按可用性展示活跃连接数 / 空态引导，让用户对「当前是否有连接」一目了然。
 */
export function ConnectionsEntryCard({ running, clashApiUrl }: ConnectionsEntryCardProps) {
  const navigate = useNavigate();
  // 可用条件与 TrafficCard 对齐：核心运行且 Clash API 已就绪。
  const available = running && clashApiUrl !== null && clashApiUrl !== "";
  const { data: active } = useQuery<ActiveConnections>({
    queryKey: CONNECTIONS_KEY,
    queryFn: connectionsActive,
    enabled: available,
    refetchInterval: available ? POLL_MS : false,
    retry: false,
  });

  const count = active?.connections.length ?? 0;
  const description = !running
    ? "启动代理后可查看连接"
    : !available
      ? "在设置中开启 Clash API 后可查看连接"
      : count > 0
        ? `${count} 个活跃连接 · 每 2 秒刷新`
        : "暂无活跃连接 · 点击查看已关闭记录";

  return (
    <EntryLinkCard
      icon={<ArrowsRightLeftIcon className="size-6" aria-hidden="true" />}
      title="当前连接"
      description={description}
      onPress={() => navigate("/connections")}
    />
  );
}
