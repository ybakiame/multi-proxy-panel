import { Chip, Spinner } from "@heroui/react";
import type { NodeView } from "@pp/client-core";
import { delayColor, delayText, nodeTypeLabel } from "./proxyFormat";

interface ProxyNodeItemProps {
  /** 节点/成员名称（分组成员名，可能不在 `nodes` 列表内）。 */
  name: string;
  /** 节点详情；成员非真实节点（如内置策略）时为 `undefined`。 */
  node?: NodeView;
  /** 是否为分组当前选中（`group.now === name`），驱动高亮。 */
  selected: boolean;
  /** 仅 Selector 分组可点击切换。 */
  selectable: boolean;
  /** 切换请求进行中（该行禁用并显示 loading）。 */
  busy: boolean;
  /** 单节点测速进行中（延迟 badge 区显示 loading）。 */
  testing: boolean;
  onSelect: () => void;
  onTest: () => void;
}

/**
 * 节点行（ADR-0003 M5，移动单列布局）。
 *
 * - 整行触达区 ≥48px：左侧主按钮（可切换时点击切换，不可切换时无操作），
 *   右侧独立延迟 badge 按钮（点击单节点测速，busy 时禁用）；
 * - 选中态高亮边框/背景（`border-primary` + `bg-primary/5`，对齐 SubscriptionSheet 选中行）；
 * - 展示：节点名 + 节点类型小字 + UDP 标 + 延迟 badge（分级色对齐 desktop NodeItem）。
 */
export function ProxyNodeItem({
  name,
  node,
  selected,
  selectable,
  busy,
  testing,
  onSelect,
  onTest,
}: ProxyNodeItemProps) {
  const canSelect = selectable && !busy && !testing;
  const testDisabled = testing || busy;

  return (
    <div
      className={`flex min-h-12 items-stretch overflow-hidden rounded-xl border transition-colors ${
        selected ? "border-primary/60 bg-primary/5" : "border-border/60 bg-surface"
      }`}
    >
      <button
        type="button"
        disabled={!canSelect}
        onClick={canSelect ? onSelect : undefined}
        aria-pressed={selectable ? selected : undefined}
        className="flex min-w-0 flex-1 items-center gap-2 px-4 py-3 text-left disabled:cursor-default disabled:opacity-100"
      >
        {busy && <Spinner size="sm" color="accent" aria-hidden="true" />}
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="flex items-center gap-1.5">
            <span className="min-w-0 truncate text-sm font-medium text-foreground">{name}</span>
            {selected && (
              <Chip size="sm" variant="soft" color="accent">
                选中
              </Chip>
            )}
          </span>
          <span className="flex items-center gap-1.5">
            {node && <span className="text-xs text-muted">{nodeTypeLabel(node.node_type)}</span>}
            {node?.udp && (
              <Chip size="sm" variant="soft" color="success">
                UDP
              </Chip>
            )}
          </span>
        </span>
      </button>

      <button
        type="button"
        onClick={testDisabled ? undefined : onTest}
        disabled={testDisabled}
        aria-label={`测速 ${name}`}
        className="flex shrink-0 items-center justify-center px-3 disabled:cursor-default"
      >
        {testing ? (
          <Spinner size="sm" color="accent" aria-hidden="true" />
        ) : (
          <Chip size="sm" variant="soft" color={delayColor(node?.delay_ms)}>
            {delayText(node?.delay_ms)}
          </Chip>
        )}
      </button>
    </div>
  );
}
