import { ArrowDownIcon, ArrowUpIcon } from "@heroicons/react/24/outline";
import { Card, Chip, Switch } from "@heroui/react";
import type { LocalRuleView } from "@pp/client-core";
import { actionLabel, matchTypeLabel, ruleSummary } from "@pp/client-core";

interface RuleCardProps {
  rule: LocalRuleView;
  index: number;
  total: number;
  /** 是否被至少一个自定义场景模板引用（未被引用的规则不注入启动配置，出「未分配模板」提示）。 */
  referencedByTemplate: boolean;
  onToggle: (next: boolean) => void;
  onMove: (dir: -1 | 1) => void;
  onEdit: () => void;
}

/** 动作 badge 配色：proxy ≈ 品牌主色 / direct = 成功 / reject = 危险（HeroUI token 无 primary，用 accent 等价表达）。 */
const ACTION_BADGE_CLASS: Record<string, string> = {
  proxy: "bg-accent/10 text-accent",
  direct: "bg-success/10 text-success",
  reject: "bg-danger/10 text-danger",
};

function OrderButton({
  label,
  disabled,
  direction,
  onPress,
}: {
  label: string;
  disabled: boolean;
  direction: "up" | "down";
  onPress: () => void;
}) {
  const Icon = direction === "up" ? ArrowUpIcon : ArrowDownIcon;
  return (
    <button
      type="button"
      aria-label={label}
      disabled={disabled}
      onClick={disabled ? undefined : onPress}
      className="flex size-12 shrink-0 items-center justify-center rounded-lg text-muted transition-opacity active:opacity-70 disabled:cursor-default disabled:opacity-35"
    >
      <Icon className="size-5" aria-hidden="true" />
    </button>
  );
}

/**
 * 单条自定义规则卡（ADR-0003 M5.4）。
 *
 * 自「场景模板改为规则引用 + 应用激活」起注入条件 = `enabled && 被已应用模板引用`：
 * - 左侧主体（规则名或「类型: 目标」摘要 + 类型/目标/动作/标记详情行）点击进编辑 Sheet；
 * - 动作 badge（代理/直连/拒绝语义色）；右侧启停 Switch——语义是**模板内启停**
 *   （规则只在所属场景模板被应用时随模板注入，禁用规则即使被模板引用也会被跳过）；
 * - 未被任何模板引用的规则出 warning「未分配模板」chip + 说明（不会注入启动配置）；
 * - 底部栏：顺序提示 + 上移/下移（边界禁用，重排由页面层回写 sort_order）。
 */
export function RuleCard({ rule, index, total, referencedByTemplate, onToggle, onMove, onEdit }: RuleCardProps) {
  const badgeClass = ACTION_BADGE_CLASS[rule.action] ?? "bg-default-soft text-muted";
  const metaDetail =
    `${matchTypeLabel(rule.match_type)}${rule.target ? `: ${rule.target}` : ""} · ${actionLabel(rule.action)}` +
    (rule.no_resolve ? " · no-resolve" : "") +
    (rule.invert ? " · invert" : "") +
    (rule.note ? ` · ${rule.note}` : "");
  const notInTemplate = !referencedByTemplate;

  return (
    <Card className="overflow-hidden">
      <div className="flex items-center gap-1 px-1 py-1 pl-0">
        <button type="button" onClick={onEdit} className="flex min-w-0 flex-1 flex-col gap-1 p-2.5 pl-2 text-left">
          <span className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-sm font-medium text-foreground">{ruleSummary(rule)}</span>
            <span className={`shrink-0 rounded-full px-2 py-0.5 text-xs font-medium leading-5 ${badgeClass}`}>
              {actionLabel(rule.action)}
            </span>
            {notInTemplate && (
              <Chip size="sm" variant="soft" color="warning" className="shrink-0">
                未分配模板
              </Chip>
            )}
          </span>
          <span className="truncate text-xs text-muted">{metaDetail}</span>
          {notInTemplate && (
            <span className="text-xs text-warning">
              未被任何场景模板引用：启用后也不会注入启动配置，请先在「场景模板」中新建并应用包含它的模板
            </span>
          )}
        </button>
        <Switch
          aria-label={`${referencedByTemplate ? "在模板中启停规则" : "启用规则"} ${ruleSummary(rule)}`}
          isSelected={rule.enabled}
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
      <div className="flex items-center gap-1 border-t border-border/40 pl-3">
        <span className="mr-auto py-1 text-xs text-muted">
          顺序 {index + 1} / {total}
        </span>
        <OrderButton label="上移规则" direction="up" disabled={index === 0} onPress={() => onMove(-1)} />
        <OrderButton label="下移规则" direction="down" disabled={index === total - 1} onPress={() => onMove(1)} />
      </div>
    </Card>
  );
}
