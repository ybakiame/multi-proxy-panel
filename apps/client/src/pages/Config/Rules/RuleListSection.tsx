import { ArrowDownIcon, ArrowUpIcon, PlusIcon } from "@heroicons/react/24/outline";
import { Button, DataList, Switch, type DataListColumn } from "@pp/ui";
import { actionLabel, ruleSummary, type LocalRuleView } from "@pp/client-core";
import { RuleCard } from "./RuleCard";

interface Props {
  rules: LocalRuleView[];
  onToggle: (rule: LocalRuleView, next: boolean) => void;
  onMove: (index: number, dir: -1 | 1) => void;
  onEdit: (rule: LocalRuleView) => void;
  onAdd: () => void;
}
export function RuleListSection({ rules, onToggle, onMove, onEdit, onAdd }: Props) {
  const columns: DataListColumn<LocalRuleView>[] = [
    {
      id: "rule",
      header: "规则",
      cell: (rule) => (
        <button
          type="button"
          onClick={() => onEdit(rule)}
          className="text-left text-accent underline-offset-2 hover:underline"
        >
          {ruleSummary(rule)}
          {rule.builtin && <span className="ml-2 text-xs text-muted">内置</span>}
        </button>
      ),
    },
    { id: "action", header: "动作", cell: (rule) => actionLabel(rule.action) },
    {
      id: "enabled",
      header: "启用",
      cell: (rule) => (
        <Switch
          aria-label={`启用规则 ${ruleSummary(rule)}`}
          isSelected={rule.enabled}
          onChange={(next) => onToggle(rule, next)}
        />
      ),
    },
    {
      id: "actions",
      header: "操作",
      cell: (rule, index) => (
        <div className="flex items-center gap-2">
          <Button variant="secondary" size="sm" onPress={() => onEdit(rule)}>
            编辑
          </Button>
          <Button
            variant="ghost"
            size="sm"
            isIconOnly
            aria-label="上移规则"
            isDisabled={index === 0}
            onPress={() => onMove(index, -1)}
          >
            <ArrowUpIcon className="size-4" />
          </Button>
          <Button
            variant="ghost"
            size="sm"
            isIconOnly
            aria-label="下移规则"
            isDisabled={index === rules.length - 1}
            onPress={() => onMove(index, 1)}
          >
            <ArrowDownIcon className="size-4" />
          </Button>
        </div>
      ),
    },
  ];
  return (
    <section className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-3">
        <p className="text-sm text-muted">启用的规则会注入启动配置；自上而下顺序匹配</p>
        <Button onPress={onAdd} size="sm">
          <PlusIcon className="size-4" />
          添加规则
        </Button>
      </div>
      <DataList
        aria-label="规则列表"
        rows={rules}
        columns={columns}
        getRowId={(rule) => rule.id}
        emptyContent="暂无规则，点击「添加规则」创建"
        renderCard={(rule, index) => (
          <RuleCard
            rule={rule}
            index={index}
            total={rules.length}
            onToggle={(next) => onToggle(rule, next)}
            onMove={(dir) => onMove(index, dir)}
            onEdit={() => onEdit(rule)}
          />
        )}
      />
    </section>
  );
}
