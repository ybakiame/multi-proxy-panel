import { PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card } from "@heroui/react";
import type { LocalRuleView } from "@pp/client-core";
import { RuleCard } from "./RuleCard";

interface RuleListSectionProps {
  rules: LocalRuleView[];
  onToggle: (rule: LocalRuleView, next: boolean) => void;
  onMove: (index: number, dir: -1 | 1) => void;
  onEdit: (rule: LocalRuleView) => void;
  onAdd: () => void;
}

/**
 * 规则页 3 区：自定义规则列表（ADR-0003 M5.4）。
 *
 * 区头「添加规则」按钮进编辑 Sheet（新建模式）；卡片列表支持启停 / 上下移 /
 * 点击编辑；无规则时展示空态引导文案。启用的规则在核心启动时全量注入。
 */
export function RuleListSection({ rules, onToggle, onMove, onEdit, onAdd }: RuleListSectionProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">自定义规则</span>
          <span className="text-xs text-muted">启用的规则会注入启动配置；自上而下顺序匹配</span>
        </div>
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加规则
        </Button>
      </div>

      {rules.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-muted">暂无自定义规则</span>
            <span className="text-xs text-muted/80">点击「添加规则」创建；建议以「最终规则」兜底或先分流域名</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {rules.map((rule, index) => (
            <RuleCard
              key={rule.id}
              rule={rule}
              index={index}
              total={rules.length}
              onToggle={(next) => onToggle(rule, next)}
              onMove={(dir) => onMove(index, dir)}
              onEdit={() => onEdit(rule)}
            />
          ))}
        </div>
      )}
    </div>
  );
}
