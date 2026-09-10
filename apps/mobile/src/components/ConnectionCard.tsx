import { Chip } from "@heroui/react";
import { XMarkIcon } from "@heroicons/react/24/outline";
import type { ConnectionView } from "@pp/client-core";
import { formatBytes } from "./TrafficCard";

interface ConnectionCardProps {
  conn: ConnectionView;
  /** 提供时渲染右上角关闭按钮（活跃列表用；已关闭记录不传）。 */
  onClose?: (id: string) => void;
  /** 该连接的关闭请求进行中（按钮转 loading 并禁用）。 */
  closing?: boolean;
}

/** 网络类型 chip 配色：tcp 绿 / udp 黄 / 其余默认（对齐 desktop Connections）。 */
function networkColor(network: string): "success" | "warning" | "default" {
  switch (network.toLowerCase()) {
    case "tcp":
      return "success";
    case "udp":
      return "warning";
    default:
      return "default";
  }
}

/** 由 start 秒级时间戳推算持续时长：≥1h 显示 `HH:MM:SS`，否则 `MM:SS`。 */
function formatDuration(start: number): string {
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - start);
  const h = Math.floor(diff / 3600);
  const m = Math.floor((diff % 3600) / 60);
  const s = diff % 60;
  const mm = m.toString().padStart(2, "0");
  const ss = s.toString().padStart(2, "0");
  if (h > 0) {
    return `${h.toString().padStart(2, "0")}:${mm}:${ss}`;
  }
  return `${mm}:${ss}`;
}

/**
 * 连接卡片（活跃 / 已关闭通用，ADR-0003 M5 移动版）。
 *
 * 单列卡片结构：目标 host（截断）+ 网络类型 chip（+ 活跃时的右上关闭按钮）、
 * 代理链（后端已按 ` → ` 连接）、命中规则（含 payload）、↑↓ 流量与连接时长。
 */
export function ConnectionCard({ conn, onClose, closing }: ConnectionCardProps) {
  const ruleText = conn.rule_payload ? `${conn.rule}（${conn.rule_payload}）` : conn.rule;
  return (
    <div className="flex flex-col gap-1.5 rounded-xl border border-border/60 bg-surface px-3 py-2.5">
      {/* 首行：目标 + 网络 chip + 关闭 */}
      <div className="flex items-center gap-2">
        <span className="min-w-0 flex-1 truncate text-sm font-medium text-foreground" title={conn.host}>
          {conn.host}
        </span>
        <Chip size="sm" variant="soft" color={networkColor(conn.network)}>
          {conn.network.toUpperCase()}
        </Chip>
        {onClose && (
          <button
            type="button"
            aria-label={`关闭连接 ${conn.host}`}
            disabled={closing}
            onClick={() => onClose(conn.id)}
            className="-mr-2 flex size-11 shrink-0 items-center justify-center rounded-lg text-muted active:bg-danger/10 active:text-danger disabled:opacity-60"
          >
            <XMarkIcon className="size-4" aria-hidden="true" />
          </button>
        )}
      </div>

      {/* 代理链 */}
      {conn.chain && (
        <p className="min-w-0 truncate text-xs text-muted" title={conn.chain}>
          链路：{conn.chain}
        </p>
      )}

      {/* 命中规则 */}
      <p className="min-w-0 truncate text-xs text-muted" title={`命中规则：${ruleText}`}>
        命中：{ruleText}
      </p>

      {/* 流量 + 时长 */}
      <div className="flex items-center justify-between gap-2 text-xs tabular-nums text-muted">
        <span className="min-w-0 truncate">
          <span className="text-danger">↑</span> {formatBytes(conn.upload)}
          <span className="mx-1.5" />
          <span className="text-success">↓</span> {formatBytes(conn.download)}
        </span>
        <span className="shrink-0 font-mono">{formatDuration(conn.start)}</span>
      </div>
    </div>
  );
}
