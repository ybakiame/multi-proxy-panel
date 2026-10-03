import { useQuery } from "@tanstack/react-query";
import { Card } from "@heroui/react";
import { useNavigate } from "react-router-dom";
import { statsToday, STATS_TODAY_KEY } from "@pp/client-core";
import { formatBytes } from "../pages/Stats/utils";

interface TodayTrafficCardProps {
  /** 核心运行中：5 秒轮询；停止时不轮询但仍展示持久化数据。 */
  running: boolean;
}

/** 仪表盘「今日流量」卡片：主指标为今日已代理流量，点击进入 /stats 详情页。 */
export default function TodayTrafficCard({ running }: TodayTrafficCardProps) {
  const navigate = useNavigate();

  const { data: today } = useQuery({
    queryKey: STATS_TODAY_KEY,
    queryFn: statsToday,
    refetchInterval: running ? 5000 : false,
    retry: false,
  });

  const proxied = (today?.proxied_upload_bytes ?? 0) + (today?.proxied_download_bytes ?? 0);

  return (
    <Card className="cursor-pointer transition-colors hover:bg-surface-secondary/40" onClick={() => navigate("/stats")}>
      <Card.Header>
        <Card.Title>今日流量</Card.Title>
        <Card.Description>{today ? `${today.date} · 点击查看详情` : "点击查看详情"}</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-3">
        <div className="flex flex-col gap-1">
          <span className="text-2xl font-semibold">{today ? formatBytes(proxied) : "-"}</span>
          <span className="text-xs text-muted">已代理流量（上行 + 下行）</span>
        </div>
        <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-medium">{today ? formatBytes(today.upload_bytes) : "-"}</span>
            <span className="text-xs text-muted">总上行</span>
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-medium">{today ? formatBytes(today.download_bytes) : "-"}</span>
            <span className="text-xs text-muted">总下行</span>
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-medium">{today ? today.connection_count : "-"}</span>
            <span className="text-xs text-muted">连接数</span>
          </div>
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-medium">{today ? today.target_count : "-"}</span>
            <span className="text-xs text-muted">目标数</span>
          </div>
        </div>
      </Card.Content>
    </Card>
  );
}
