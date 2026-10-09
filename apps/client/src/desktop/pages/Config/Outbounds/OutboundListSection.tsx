import { Button, Chip, Switch } from "@heroui/react";
import { PlusIcon } from "@heroicons/react/24/outline";
import { isGroupOutbound, outboundTag } from "@pp/client-core";
import type { CustomOutbound } from "@pp/client-core";
import { groupSummary, outboundProtocolLabel, outboundSummary } from "@pp/client-core";

interface OutboundListSectionProps {
  items: CustomOutbound[];
  /** 与 `items` 等长的逐项校验错误（页面草稿校验结果）。 */
  itemErrors: (string | null)[];
  onToggle: (item: CustomOutbound, next: boolean) => void;
  onEdit: (item: CustomOutbound) => void;
  /** 添加分组（预选 selector）。 */
  onAddGroup: () => void;
  /** 添加节点（预选 vless）。 */
  onAddNode: () => void;
}

interface OutboundRow {
  item: CustomOutbound;
  error: string | null;
}

/** 单条出站行：名称 / 协议标签 / 摘要 / tag / 启停开关。 */
function OutboundRowCard({
  row,
  onToggle,
  onEdit,
}: {
  row: OutboundRow;
  onToggle: (item: CustomOutbound, next: boolean) => void;
  onEdit: (item: CustomOutbound) => void;
}) {
  const { item, error } = row;
  return (
    <div className="flex items-center gap-2 rounded-lg border border-border/60 bg-surface-secondary/40 p-3">
      <button
        type="button"
        onClick={() => onEdit(item)}
        aria-label={`编辑出站 ${item.name || item.id}`}
        className="flex min-w-0 flex-1 flex-col gap-1 text-left"
      >
        <span className="flex min-w-0 items-center gap-2">
          <span className="min-w-0 truncate text-sm font-medium text-foreground">{item.name || "未命名"}</span>
          <Chip size="sm" variant="soft" color="accent" className="shrink-0">
            {outboundProtocolLabel(item.type)}
          </Chip>
          {item.builtin === true && (
            <Chip size="sm" variant="soft" color="default" className="shrink-0">
              内置
            </Chip>
          )}
        </span>
        <span className="truncate text-xs text-muted">
          {isGroupOutbound(item) ? groupSummary(item) : outboundSummary(item)}
        </span>
        <span className="truncate font-mono text-xs text-muted/70">
          {item.builtin === true ? item.name : outboundTag(item.name)}
        </span>
        {error && <span className="truncate text-xs text-amber-500">{error}</span>}
      </button>
      <Switch
        size="sm"
        aria-label={`启用出站 ${item.name || item.id}`}
        isSelected={item.enabled}
        onChange={(next) => onToggle(item, next)}
      >
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Content>
      </Switch>
    </div>
  );
}

/** 一个出站分区（区头 + 添加按钮 + 独立空态 + 行列表）。 */
function OutboundSection({
  title,
  subtitle,
  emptyHint,
  addLabel,
  rows,
  onAdd,
  onToggle,
  onEdit,
}: {
  title: string;
  subtitle: string;
  emptyHint: string;
  addLabel: string;
  rows: OutboundRow[];
  onAdd: () => void;
  onToggle: (item: CustomOutbound, next: boolean) => void;
  onEdit: (item: CustomOutbound) => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">{title}</span>
          <span className="truncate text-xs text-muted">{subtitle}</span>
        </div>
        <Button size="sm" variant="primary" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          {addLabel}
        </Button>
      </div>

      {rows.length === 0 ? (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          {emptyHint}
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {rows.map((row) => (
            <OutboundRowCard key={row.item.id} row={row} onToggle={onToggle} onEdit={onEdit} />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * 自定义出站页列表区（桌面端；语义对齐移动端 `OutboundListSection`）：分为「分组」
 * （selector / urltest）与「节点」两个分区，分组在上，各自独立空态与添加入口。
 */
export function OutboundListSection({
  items,
  itemErrors,
  onToggle,
  onEdit,
  onAddGroup,
  onAddNode,
}: OutboundListSectionProps) {
  const rows: OutboundRow[] = items.map((item, index) => ({ item, error: itemErrors[index] ?? null }));
  const groups = rows.filter((row) => isGroupOutbound(row.item));
  const nodes = rows.filter((row) => !isGroupOutbound(row.item));

  return (
    <div className="flex flex-col gap-5">
      <OutboundSection
        title="分组"
        subtitle="selector / urltest"
        emptyHint="暂无分组，添加后可让规则引用一组节点"
        addLabel="添加分组"
        rows={groups}
        onAdd={onAddGroup}
        onToggle={onToggle}
        onEdit={onEdit}
      />
      <OutboundSection
        title="节点"
        subtitle="vless / vmess / shadowsocks / trojan / hysteria2"
        emptyHint="暂无自定义出站；自定义出站可被「规则」页的出站动作引用"
        addLabel="添加节点"
        rows={nodes}
        onAdd={onAddNode}
        onToggle={onToggle}
        onEdit={onEdit}
      />
    </div>
  );
}
