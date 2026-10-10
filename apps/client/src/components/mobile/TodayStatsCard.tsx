import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { ChevronRightIcon } from "@heroicons/react/24/outline";
import { STATS_TODAY_KEY, statsToday } from "@pp/client-core";
import type { TodaySummary } from "@pp/client-core";
import { Card } from "@pp/ui";
import { formatBytes } from "./TrafficCard";

interface TodayStatsCardProps {
  /** 核心是否运行（运行中 5 秒轮询刷新；未运行不轮询，仅展示持久化数据）。 */
  running: boolean;
}

/** 轮询间隔（毫秒）。 */
const POLL_MS = 5000;

/**
 * 今日流量卡（持久化统计，数据源 `stats_today` → 本地 SQLite）。
 *
 * - 主指标：今日已代理流量（proxied 上下行合计，排除直连/拦截）；
 * - 次指标：总上行 / 总下行 / 连接数 / 目标数；
 * - 与 TrafficCard（Clash API 会话实时统计）不同：本卡数据落盘持久化，
 *   核心未运行时仍展示历史数据，仅运行中开启 5 秒轮询；
 * - 整卡可点击，跳转流量统计详情页 `/stats`。
 */
export function TodayStatsCard({ running }: TodayStatsCardProps) {
  const navigate = useNavigate();
  const { data } = useQuery<TodaySummary>({
    queryKey: STATS_TODAY_KEY,
    queryFn: statsToday,
    refetchInterval: running ? POLL_MS : false,
    retry: false,
  });

  const proxiedTotal = (data?.proxied_upload_bytes ?? 0) + (data?.proxied_download_bytes ?? 0);
  const metrics: Array<[string, string]> = [
    ["总上行", formatBytes(data?.upload_bytes ?? 0)],
    ["总下行", formatBytes(data?.download_bytes ?? 0)],
    ["连接数", String(data?.connection_count ?? 0)],
    ["目标数", String(data?.target_count ?? 0)],
  ];

  return (
    <button
      type="button"
      aria-label="今日流量，点击查看统计详情"
      onClick={() => void navigate("/stats")}
      className="block w-full rounded-2xl text-left transition-opacity active:opacity-80"
    >
      <Card>
        <Card.Header>
          <Card.Title>今日流量</Card.Title>
          <Card.Description>已代理流量（排除直连与拦截），点击进入详情</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <span className="text-2xl font-semibold tabular-nums text-zinc-900 dark:text-zinc-100">
            {formatBytes(proxiedTotal)}
          </span>
          <div className="grid grid-cols-2 gap-4">
            {metrics.map(([label, value]) => (
              <div key={label} className="flex flex-col gap-0.5">
                <span className="text-xs text-zinc-500 dark:text-zinc-400">{label}</span>
                <span className="text-base font-medium tabular-nums text-zinc-900 dark:text-zinc-100">{value}</span>
              </div>
            ))}
          </div>
          <span className="flex items-center gap-0.5 self-end text-xs text-primary">
            查看详情
            <ChevronRightIcon className="size-3.5" aria-hidden="true" />
          </span>
        </Card.Content>
      </Card>
    </button>
  );
}
