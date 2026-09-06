import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Card } from "@heroui/react";
import { CLOSED_CONNECTIONS_KEY, CONNECTIONS_KEY, connectionsActive, connectionsClosed } from "@pp/client-core";
import type { ActiveConnections, ConnectionView } from "@pp/client-core";

interface TrafficCardProps {
  /** 核心是否运行。 */
  running: boolean;
  /** 运行中且 Clash API 开启时的面板地址；核心未运行或 API 关闭时为 `null`。 */
  clashApiUrl: string | null;
}

/** 轮询间隔（毫秒，与 desktop Connections 页同频）。 */
const POLL_MS = 2000;
/** 计算瞬时速度所需的最小采样间隔（毫秒），跳过同轮内两次 resolve 的短步进以免尖峰。 */
const MIN_SPEED_WINDOW_MS = 1000;

/** 字节速率格式化：B/s / KB/s / MB/s（≥1024 进位）。 */
export function formatRate(bps: number): string {
  const value = Number.isFinite(bps) && bps > 0 ? bps : 0;
  if (value >= 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(2)} MB/s`;
  }
  if (value >= 1024) {
    return `${(value / 1024).toFixed(1)} KB/s`;
  }
  return `${value.toFixed(0)} B/s`;
}

/** 字节总量格式化：B / KB / MB / GB（≥1024 进位）。 */
export function formatBytes(bytes: number): string {
  const value = Number.isFinite(bytes) && bytes > 0 ? bytes : 0;
  if (value < 1024) {
    return `${value.toFixed(0)} B`;
  }
  const units = ["KB", "MB", "GB", "TB"];
  const i = Math.min(Math.floor(Math.log(value) / Math.log(1024)) - 1, units.length - 1);
  return `${(value / 1024 ** (i + 1)).toFixed(2)} ${units[i]}`;
}

/**
 * 流量统计卡（ADR-0003 M5）。
 *
 * - 数据：`connections_active` + `connections_closed` 每 2s 轮询（Query 缓存，见 keys.ts）；
 * - 会话总量 = active.upload_total + Σ closed.upload（download 同理）；
 * - 当前速度 = 相邻两次轮询总量差 / 间隔（上次快照存于 ref，随数据刷新在 effect 内推进）；
 * - 空态：核心未运行或 Clash API 未开启时提示，不发起轮询。
 */
export function TrafficCard({ running, clashApiUrl }: TrafficCardProps) {
  const available = running && clashApiUrl !== null && clashApiUrl !== "";
  const { data: active } = useQuery<ActiveConnections>({
    queryKey: CONNECTIONS_KEY,
    queryFn: connectionsActive,
    enabled: available,
    refetchInterval: available ? POLL_MS : false,
    retry: false,
  });
  const { data: closed } = useQuery<ConnectionView[]>({
    queryKey: CLOSED_CONNECTIONS_KEY,
    queryFn: connectionsClosed,
    enabled: available,
    refetchInterval: available ? POLL_MS : false,
    retry: false,
  });

  const totals = useMemo(() => {
    const upload = (active?.upload_total ?? 0) + (closed ?? []).reduce((sum, conn) => sum + conn.upload, 0);
    const download = (active?.download_total ?? 0) + (closed ?? []).reduce((sum, conn) => sum + conn.download, 0);
    return { upload, download };
  }, [active, closed]);

  // 当前速度：相邻两次轮询总量差 / 间隔。活跃/关闭两个 Query 同频 2s 刷新，
  // effect 在任一数据更新时推进快照；与上次快照间隔 <1s 视为同轮内第二次 resolve，跳过免尖峰。
  const [speed, setSpeed] = useState({ upload: 0, download: 0 });
  const snapshotRef = useRef<{ t: number; upload: number; download: number } | null>(null);
  useEffect(() => {
    if (!available) {
      snapshotRef.current = null;
      return;
    }
    const now = Date.now();
    const prev = snapshotRef.current;
    if (prev && now - prev.t >= MIN_SPEED_WINDOW_MS) {
      const dt = (now - prev.t) / 1000;
      if (dt > 0) {
        setSpeed({
          upload: Math.max(0, (totals.upload - prev.upload) / dt),
          download: Math.max(0, (totals.download - prev.download) / dt),
        });
      }
    }
    snapshotRef.current = { t: now, upload: totals.upload, download: totals.download };
  }, [available, totals]);

  const metrics: Array<[string, string]> = [
    ["上行速度", formatRate(speed.upload)],
    ["下行速度", formatRate(speed.download)],
    ["会话上行", formatBytes(totals.upload)],
    ["会话下行", formatBytes(totals.download)],
  ];

  return (
    <Card>
      <Card.Header>
        <Card.Title>流量统计</Card.Title>
        <Card.Description>
          {available ? "会话累计与实时速度（每 2 秒刷新）" : "来自 Clash API 的实时统计"}
        </Card.Description>
      </Card.Header>
      <Card.Content>
        {!available ? (
          <div className="flex flex-col items-center justify-center gap-1.5 py-8 text-center">
            <span className="text-sm text-muted">在设置中开启 Clash API 后可查看流量统计</span>
            {!running && <span className="text-xs text-muted/80">启动代理后流量统计才会开始记录</span>}
          </div>
        ) : (
          <div className="grid grid-cols-2 gap-4">
            {metrics.map(([label, value]) => (
              <div key={label} className="flex flex-col gap-0.5">
                <span className="text-xs text-muted">{label}</span>
                <span className="text-base font-semibold tabular-nums">{value}</span>
              </div>
            ))}
          </div>
        )}
      </Card.Content>
    </Card>
  );
}
