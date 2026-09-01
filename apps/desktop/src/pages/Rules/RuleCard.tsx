import { Button, Switch } from "@heroui/react";
import { ArrowDownIcon, ArrowUpIcon, ShieldCheckIcon, TrashIcon } from "@heroicons/react/24/outline";
import type { LocalRuleView } from "../../api";
import { ruleDetailLine, ruleSummary } from "./types";

export interface RuleCardProps {
  rule: LocalRuleView;
  index: number;
  total: number;
  onToggle: (id: string) => void;
  onMoveUp: (index: number) => void;
  onMoveDown: (index: number) => void;
  onEdit: (rule: LocalRuleView) => void;
  onDelete: (rule: LocalRuleView) => void;
}

export function RuleCard({ rule, index, total, onToggle, onMoveUp, onMoveDown, onEdit, onDelete }: RuleCardProps) {
  return (
    <div className="flex items-center gap-2 rounded-lg border border-border/60 bg-surface-secondary/40 p-3 transition-colors">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-foreground">{ruleSummary(rule)}</span>
          {rule.note && <span className="truncate text-xs text-muted">({rule.note})</span>}
        </div>
        <span className="text-xs text-muted">
          {rule.match_type}: {rule.target} {ruleDetailLine(rule)}
        </span>
      </div>
      <div className="flex shrink-0 items-center gap-1">
        <Switch
          size="sm"
          isSelected={rule.enabled}
          onChange={() => onToggle(rule.id)}
          aria-label={`启用规则 ${ruleSummary(rule)}`}
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
          </Switch.Content>
        </Switch>
        <Button
          size="sm"
          variant="ghost"
          isIconOnly
          isDisabled={index === 0}
          aria-label="上移"
          onPress={() => onMoveUp(index)}
        >
          <ArrowUpIcon className="size-4" />
        </Button>
        <Button
          size="sm"
          variant="ghost"
          isIconOnly
          isDisabled={index === total - 1}
          aria-label="下移"
          onPress={() => onMoveDown(index)}
        >
          <ArrowDownIcon className="size-4" />
        </Button>
        <Button size="sm" variant="ghost" isIconOnly aria-label="编辑" onPress={() => onEdit(rule)}>
          <ShieldCheckIcon className="size-4" />
        </Button>
        <Button size="sm" variant="ghost" isIconOnly aria-label="删除" onPress={() => onDelete(rule)}>
          <TrashIcon className="size-4 text-danger" />
        </Button>
      </div>
    </div>
  );
}
