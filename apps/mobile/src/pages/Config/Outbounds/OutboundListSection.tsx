import { PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card, Switch } from "@heroui/react";
import { isGroupOutbound, outboundTag } from "@pp/client-core";
import type { CustomOutbound } from "@pp/client-core";
import { outboundSummary } from "./outboundForm";
import { groupSummary } from "./groupForm";
import { outboundProtocolLabel } from "./outboundOptions";

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

/** 单条出站卡片：名称 / 协议标签 / 摘要 / tag / 启停开关。 */
function OutboundCard({
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
    <Card className="overflow-hidden">
      <div className="flex items-center gap-1 px-1 py-1 pl-0">
        <button
          type="button"
          onClick={() => onEdit(item)}
          aria-label={`编辑出站 ${item.name || item.id}`}
          className="flex min-w-0 flex-1 flex-col gap-1 p-2.5 pl-3 text-left active:opacity-80"
        >
          <span className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-sm font-medium text-foreground">{item.name || "未命名"}</span>
            <span className="shrink-0 rounded-full bg-accent/10 px-2 py-0.5 text-xs font-medium leading-5 text-accent">
              {outboundProtocolLabel(item.type)}
            </span>
          </span>
          <span className="truncate text-xs text-muted">
            {isGroupOutbound(item) ? groupSummary(item) : outboundSummary(item)}
          </span>
          <span className="truncate font-mono text-xs text-muted/80">{outboundTag(item.name)}</span>
          {error && <span className="truncate text-xs text-warning">{error}</span>}
        </button>
        <Switch
          aria-label={`启用出站 ${item.name || item.id}`}
          isSelected={item.enabled}
          onChange={(next) => onToggle(item, next)}
          className="shrink-0 px-1"
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch.Content>
        </Switch>
      </div>
    </Card>
  );
}

/** 一个出站分区（区头 + 添加按钮 + 独立空态 + 卡片列表）。 */
function OutboundSection({
  title,
  subtitle,
  emptyTitle,
  emptyHint,
  addLabel,
  rows,
  onAdd,
  onToggle,
  onEdit,
}: {
  title: string;
  subtitle: string;
  emptyTitle: string;
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
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          {addLabel}
        </Button>
      </div>

      {rows.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-muted">{emptyTitle}</span>
            <span className="text-xs text-muted/80">{emptyHint}</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {rows.map((row) => (
            <OutboundCard key={row.item.id} row={row} onToggle={onToggle} onEdit={onEdit} />
          ))}
        </div>
      )}
    </div>
  );
}

/**
 * 自定义出站页列表区（ADR-0005 P0-4c）：分为「分组」（selector / urltest）与
 * 「节点」两个分区，分组在上，各自独立空态与添加入口。
 *
 * 卡片展示名称 / 协议标签 / 摘要（节点为 server:port，分组为成员数 + 成员摘要）/
 * 渲染 tag，点击进编辑 Sheet，右侧启停开关即时切换（仅更新内存草稿）。
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
        emptyTitle="暂无分组"
        emptyHint="添加后可让规则引用一组节点"
        addLabel="添加分组"
        rows={groups}
        onAdd={onAddGroup}
        onToggle={onToggle}
        onEdit={onEdit}
      />
      <OutboundSection
        title="节点"
        subtitle="vless / vmess / shadowsocks / trojan / hysteria2"
        emptyTitle="暂无自定义出站"
        emptyHint="自定义出站可被「自定义规则」的出站动作引用"
        addLabel="添加节点"
        rows={nodes}
        onAdd={onAddNode}
        onToggle={onToggle}
        onEdit={onEdit}
      />
    </div>
  );
}
