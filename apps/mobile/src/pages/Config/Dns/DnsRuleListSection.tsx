import { PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card, Switch } from "@heroui/react";
import type { DnsRule } from "@pp/client-core";
import { dnsRuleSummary } from "./dnsUtils";

interface DnsRuleListSectionProps {
  rules: DnsRule[];
  onToggle: (rule: DnsRule, next: boolean) => void;
  onEdit: (rule: DnsRule) => void;
  onAdd: () => void;
}

/**
 * DNS 页 4 区：DNS 分流规则列表（ADR-0005 P0-4b）。
 *
 * 区头「添加规则」按钮进编辑 Sheet；卡片主体点击进编辑 Sheet，右侧启停开关即时
 * 切换（仅更新内存草稿）。自上而下顺序匹配，删除入口在 Sheet 内。
 */
export function DnsRuleListSection({ rules, onToggle, onEdit, onAdd }: DnsRuleListSectionProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">DNS 分流规则</span>
          <span className="text-xs text-muted">自上而下匹配，命中后使用指定服务器解析</span>
        </div>
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加规则
        </Button>
      </div>

      {rules.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-muted">暂无 DNS 分流规则</span>
            <span className="text-xs text-muted/80">未匹配规则的查询走 final 服务器</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {rules.map((rule) => (
            <Card key={rule.id} className="overflow-hidden">
              <div className="flex items-center gap-1 px-1 py-1 pl-0">
                <button
                  type="button"
                  onClick={() => onEdit(rule)}
                  aria-label={`编辑规则 ${dnsRuleSummary(rule)}`}
                  className="flex min-w-0 flex-1 flex-col gap-1 p-2.5 pl-3 text-left active:opacity-80"
                >
                  <span className="min-w-0 truncate text-sm font-medium text-foreground">{rule.target}</span>
                  <span className="truncate text-xs text-muted">{dnsRuleSummary(rule)}</span>
                </button>
                <Switch
                  aria-label={`启用规则 ${dnsRuleSummary(rule)}`}
                  isSelected={rule.enabled}
                  onChange={(next) => onToggle(rule, next)}
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
          ))}
        </div>
      )}
    </div>
  );
}
