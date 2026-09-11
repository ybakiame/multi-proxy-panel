import { PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card, Switch } from "@heroui/react";
import { outboundTag } from "@pp/client-core";
import type { CustomOutbound } from "@pp/client-core";
import { outboundSummary } from "./outboundForm";
import { outboundProtocolLabel } from "./outboundOptions";

interface OutboundListSectionProps {
  items: CustomOutbound[];
  /** 与 `items` 等长的逐项校验错误（页面草稿校验结果）。 */
  itemErrors: (string | null)[];
  onToggle: (item: CustomOutbound, next: boolean) => void;
  onEdit: (item: CustomOutbound) => void;
  onAdd: () => void;
}

/**
 * 自定义出站页 2 区：出站列表（ADR-0005 P0-4c）。
 *
 * 区头「添加出站」按钮进编辑 Sheet；卡片展示名称 / 协议标签 / server:port 摘要 /
 * 渲染 tag，点击进编辑 Sheet，右侧启停开关即时切换（仅更新内存草稿）。
 * 无出站时展示空态，说明其可被「自定义规则」的出站动作引用。
 */
export function OutboundListSection({ items, itemErrors, onToggle, onEdit, onAdd }: OutboundListSectionProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">自定义出站</span>
          <span className="text-xs text-muted">vless / vmess / shadowsocks / trojan / hysteria2</span>
        </div>
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加出站
        </Button>
      </div>

      {items.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-muted">暂无自定义出站</span>
            <span className="text-xs text-muted/80">自定义出站可被「自定义规则」的出站动作引用</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {items.map((item, index) => {
            const error = itemErrors[index] ?? null;
            return (
              <Card key={item.id} className="overflow-hidden">
                <div className="flex items-center gap-1 px-1 py-1 pl-0">
                  <button
                    type="button"
                    onClick={() => onEdit(item)}
                    aria-label={`编辑出站 ${item.name || item.id}`}
                    className="flex min-w-0 flex-1 flex-col gap-1 p-2.5 pl-3 text-left active:opacity-80"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <span className="min-w-0 truncate text-sm font-medium text-foreground">
                        {item.name || "未命名"}
                      </span>
                      <span className="shrink-0 rounded-full bg-accent/10 px-2 py-0.5 text-xs font-medium leading-5 text-accent">
                        {outboundProtocolLabel(item.type)}
                      </span>
                    </span>
                    <span className="truncate text-xs text-muted">{outboundSummary(item)}</span>
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
          })}
        </div>
      )}
    </div>
  );
}
