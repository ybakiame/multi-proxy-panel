import { Button, Chip } from "@heroui/react";
import type { NodeView } from "../../api";

interface NodeItemProps {
  name: string;
  node: NodeView | undefined;
  selected: boolean;
  selectable: boolean;
  isAuto: boolean;
  busy: boolean;
  testing: boolean;
  onSelect: () => void;
  onTest: () => void;
}

/** 延迟色块颜色：绿 < 300ms，黄 < 800ms，灰 = 超时/未测。 */
function delayColor(delay: number | null | undefined): "success" | "warning" | "default" {
  if (delay == null) return "default";
  if (delay < 300) return "success";
  if (delay < 800) return "warning";
  return "default";
}

/** 延迟展示文本。 */
function delayText(delay: number | null | undefined): string {
  if (delay == null) return "超时";
  return `${delay}ms`;
}

/** 节点类型中文标签（常见类型映射）。 */
function nodeTypeLabel(type: string | undefined): string {
  if (!type) return "未知";
  const map: Record<string, string> = {
    Shadowsocks: "SS",
    Vmess: "VMess",
    Trojan: "Trojan",
    Vless: "VLESS",
    Hysteria: "Hysteria",
    Hysteria2: "Hy2",
    Tuic: "Tuic",
    WireGuard: "WG",
    Direct: "直连",
    Reject: "拒绝",
    Http: "HTTP",
    Socks5: "SOCKS5",
    Snell: "Snell",
  };
  return map[type] ?? type;
}

export default function NodeItem({
  name,
  node,
  selected,
  selectable,
  isAuto,
  busy,
  testing,
  onSelect,
  onTest,
}: NodeItemProps) {
  const canClick = selectable && !busy && !testing;

  return (
    <button
      type="button"
      className={[
        "flex w-full items-center justify-between gap-2 rounded-lg border px-3 py-2 text-left transition-colors",
        selected
          ? "border-primary bg-primary/10"
          : "border-border/60 bg-surface hover:border-primary/40 hover:bg-surface-secondary",
        canClick ? "cursor-pointer" : "cursor-default",
      ].join(" ")}
      onClick={() => {
        if (canClick) onSelect();
      }}
      disabled={!canClick}
      aria-pressed={selectable ? selected : undefined}
      aria-label={selectable ? `选择节点 ${name}` : undefined}
    >
      <div className="flex min-w-0 flex-col gap-0.5">
        <div className="flex items-center gap-1.5">
          <span className="truncate text-sm font-medium">{name}</span>
          {selected && (
            <Chip size="sm" variant="soft" color="accent">
              选中
            </Chip>
          )}
          {isAuto && selected && (
            <Chip size="sm" variant="soft" color="default">
              自动
            </Chip>
          )}
        </div>
        <div className="flex items-center gap-1.5">
          {node && (
            <Chip size="sm" variant="soft" color="default">
              {nodeTypeLabel(node.node_type)}
            </Chip>
          )}
          {node?.udp && (
            <Chip size="sm" variant="soft" color="success">
              UDP
            </Chip>
          )}
        </div>
      </div>

      <Button
        size="sm"
        variant="ghost"
        isIconOnly
        isPending={testing}
        isDisabled={testing}
        onPress={() => {
          onTest();
        }}
        aria-label={`测速 ${name}`}
        className="shrink-0"
      >
        <Chip size="sm" variant="soft" color={delayColor(node?.delay_ms)}>
          {testing ? "测速中" : delayText(node?.delay_ms)}
        </Chip>
      </Button>
    </button>
  );
}
