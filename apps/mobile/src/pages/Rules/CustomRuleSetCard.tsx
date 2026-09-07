import { Chip, Card, Switch } from "@heroui/react";
import { PencilSquareIcon, TrashIcon } from "@heroicons/react/24/outline";
import type { CustomRuleSetView } from "@pp/client-core";

interface CustomRuleSetCardProps {
  ruleSet: CustomRuleSetView;
  /** 启停写操作进行中（单飞，禁用 Switch）。 */
  busy: boolean;
  onToggle: (next: boolean) => void;
  onEdit: () => void;
  onDelete: () => void;
}

/** 来源类型 label（对齐 CustomRuleSetSource）。 */
function sourceLabel(ruleSet: CustomRuleSetView): string {
  if (ruleSet.source.kind === "manual") return "手动 JSON";
  return ruleSet.source.format === "binary" ? "远程 srs" : "远程 json";
}

function formatUpdated(lastUpdated: number): string {
  if (lastUpdated <= 0) return "从未更新";
  return new Date(lastUpdated * 1000).toLocaleString();
}

/**
 * 自定义规则集卡（ADR-0003 M5.4 规则集管理子页）。
 *
 * 展示名称 / tag / 来源类型 chip / 缓存状态 chip / 更新时间；右侧启停 Switch，
 * 底部「编辑 / 删除」入口。删除经父层 AlertDialog 确认。
 */
export function CustomRuleSetCard({ ruleSet, busy, onToggle, onEdit, onDelete }: CustomRuleSetCardProps) {
  const { name, tag, cached, enabled } = ruleSet;
  return (
    <Card>
      <div className="flex items-center gap-1 px-2 py-1 pl-0">
        <div className="flex min-w-0 flex-1 flex-col gap-0.5 px-2 py-2 pl-1">
          <span className="flex min-w-0 items-center gap-1.5">
            <span className="min-w-0 truncate text-sm font-medium text-foreground">{name.trim() || tag}</span>
            <Chip size="sm" variant="soft" color="accent" className="shrink-0">
              {sourceLabel(ruleSet)}
            </Chip>
          </span>
          <span className="truncate text-xs text-muted">
            规则集引用名：<span className="font-mono text-foreground/80">{tag}</span>
          </span>
          <span className="flex min-w-0 items-center gap-1.5 text-xs text-muted">
            {cached ? (
              <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                已缓存
              </Chip>
            ) : (
              <Chip size="sm" variant="soft" color="default" className="shrink-0">
                未缓存
              </Chip>
            )}
            <span className="shrink-0">更新于 {formatUpdated(ruleSet.last_updated)}</span>
          </span>
        </div>
        <Switch
          aria-label={`启用规则集 ${tag}`}
          isSelected={enabled}
          isDisabled={busy}
          onChange={(next) => onToggle(next)}
          className="shrink-0 px-1"
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch.Content>
        </Switch>
      </div>
      <div className="flex items-center justify-end gap-1 border-t border-border/40 py-0.5 pr-1">
        <button
          type="button"
          onClick={onEdit}
          disabled={busy}
          className="flex min-h-11 shrink-0 items-center gap-1 rounded-lg px-3 text-sm text-foreground active:opacity-70 disabled:opacity-40"
        >
          <PencilSquareIcon className="size-4" aria-hidden="true" />
          编辑
        </button>
        <button
          type="button"
          onClick={onDelete}
          disabled={busy}
          className="flex min-h-11 shrink-0 items-center gap-1 rounded-lg px-3 text-sm text-danger active:opacity-70 disabled:opacity-40"
        >
          <TrashIcon className="size-4" aria-hidden="true" />
          删除
        </button>
      </div>
    </Card>
  );
}
