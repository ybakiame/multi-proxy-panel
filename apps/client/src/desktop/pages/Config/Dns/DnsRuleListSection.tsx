import { Button, Switch } from "@heroui/react";
import { PlusIcon } from "@heroicons/react/24/outline";
import type { DnsRule } from "@pp/client-core";
import { dnsRuleSummary } from "@pp/client-core";

interface DnsRuleListSectionProps {
  rules: DnsRule[];
  onToggle: (rule: DnsRule, next: boolean) => void;
  onEdit: (rule: DnsRule) => void;
  onAdd: () => void;
}

/**
 * DNS 页分流规则列表区（桌面端；语义对齐移动端 `DnsRuleListSection`）。
 *
 * 区头「添加规则」进编辑弹窗；卡片主体点击进编辑弹窗，右侧启停开关即时切换
 * （仅更新内存草稿）。自上而下顺序匹配，删除入口在编辑弹窗内。
 */
export function DnsRuleListSection({ rules, onToggle, onEdit, onAdd }: DnsRuleListSectionProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">DNS 分流规则</span>
          <span className="text-xs text-muted">自上而下匹配，命中后使用指定服务器解析</span>
        </div>
        <Button size="sm" variant="primary" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加规则
        </Button>
      </div>

      {rules.length === 0 ? (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          暂无 DNS 分流规则，未匹配规则的查询走 final 服务器
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {rules.map((rule) => (
            <div
              key={rule.id}
              className="flex items-center gap-2 rounded-lg border border-border/60 bg-surface-secondary/40 p-3"
            >
              <button
                type="button"
                onClick={() => onEdit(rule)}
                aria-label={`编辑规则 ${dnsRuleSummary(rule)}`}
                className="flex min-w-0 flex-1 flex-col gap-1 text-left"
              >
                <span className="min-w-0 truncate text-sm font-medium text-foreground">
                  {rule.target || dnsRuleSummary(rule)}
                </span>
                <span className="truncate text-xs text-muted">{dnsRuleSummary(rule)}</span>
              </button>
              <Switch
                size="sm"
                aria-label={`启用规则 ${dnsRuleSummary(rule)}`}
                isSelected={rule.enabled}
                onChange={(next) => onToggle(rule, next)}
              >
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                </Switch.Content>
              </Switch>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
