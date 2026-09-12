import type { ReactNode } from "react";
import { Card, Chip } from "@heroui/react";
import type { BaselineOutboundView, BaselineRuleSetView, BaselineRuleView } from "@pp/client-core";

/**
 * 内置 CN 分流基线只读展示组件（T3b）。
 *
 * 三个分区共用同一视觉语言：区头「内置」+ 计数 + 语境说明，条目为只读卡片并带
 * 「内置」徽章。所有条目均无点击 / 编辑 / 删除 / 开关，仅作基线信息展示。
 */

/** 「内置」只读徽章：统一标识基线项不可编辑、无开关。 */
function BuiltinBadge() {
  return (
    <Chip size="sm" variant="soft" color="default" className="shrink-0">
      内置
    </Chip>
  );
}

/** 内置只读分区外壳：区头「内置」+ 计数 + 说明 + 只读卡片列表。 */
function BaselineSection({ count, hint, children }: { count: number; hint: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <span className="min-w-0 truncate text-sm font-medium text-foreground">内置</span>
        <Chip size="sm" variant="soft" color="default" className="shrink-0">
          {count}
        </Chip>
      </div>
      <span className="text-xs text-muted">{hint}</span>
      <div className="flex flex-col gap-2">{children}</div>
    </section>
  );
}

/** 内置路由规则只读分区（规则管理页置底：用户规则先于基线生效）。 */
export function BaselineRuleSection({ rules }: { rules: BaselineRuleView[] }) {
  return (
    <BaselineSection count={rules.length} hint="内置基线始终生效、不可编辑；上方用户规则优先于内置规则。">
      {rules.map((rule, index) => (
        <Card key={`${rule.outbound}-${index}`} className="overflow-hidden">
          <div className="flex items-center justify-between gap-2 px-4 py-3">
            <span className="min-w-0 truncate text-sm text-foreground">{rule.description}</span>
            <BuiltinBadge />
          </div>
        </Card>
      ))}
    </BaselineSection>
  );
}

/** 内置规则集只读分区（规则集管理页置底）。 */
export function BaselineRuleSetSection({ ruleSets }: { ruleSets: BaselineRuleSetView[] }) {
  return (
    <BaselineSection count={ruleSets.length} hint="内置规则集始终可用、不可编辑；是否被引用由用户规则决定。">
      {ruleSets.map((ruleSet) => (
        <Card key={ruleSet.tag} className="overflow-hidden">
          <div className="flex items-center justify-between gap-2 px-4 py-3">
            <div className="flex min-w-0 flex-col gap-0.5">
              <span className="flex min-w-0 items-center gap-1.5">
                <span className="min-w-0 truncate text-sm font-medium text-foreground">{ruleSet.description}</span>
                <span className="shrink-0 font-mono text-xs text-muted">{ruleSet.tag}</span>
              </span>
              <span className="truncate text-xs text-muted/80">{ruleSet.url}</span>
            </div>
            <BuiltinBadge />
          </div>
        </Card>
      ))}
    </BaselineSection>
  );
}

/** 内置出站只读分区（自定义出站页置底）。 */
export function BaselineOutboundSection({ outbounds }: { outbounds: BaselineOutboundView[] }) {
  return (
    <BaselineSection count={outbounds.length} hint="内置出站始终可用、不可编辑；可作为规则动作或 route.final 目标。">
      {outbounds.map((outbound) => (
        <Card key={outbound.tag} className="overflow-hidden">
          <div className="flex items-center justify-between gap-2 px-4 py-3">
            <span className="flex min-w-0 items-center gap-1.5">
              <span className="min-w-0 truncate text-sm text-foreground">
                <span className="font-mono">{outbound.tag}</span>
                <span className="mx-1 text-muted">·</span>
                {outbound.description}
              </span>
              <span className="shrink-0 rounded-full bg-default-soft px-2 py-0.5 text-xs leading-5 text-muted">
                {outbound.kind}
              </span>
            </span>
            <BuiltinBadge />
          </div>
        </Card>
      ))}
    </BaselineSection>
  );
}
