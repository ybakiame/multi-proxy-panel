import { ArrowDownIcon, ArrowUpIcon } from "@heroicons/react/24/outline";
import { Card, Chip, Switch } from "@heroui/react";
import type { LocalRuleView } from "@pp/client-core";
import {
  actionLabel,
  formatRuleSetTarget,
  isOutboundAction,
  matchTypeLabel,
  outboundTagFromAction,
  ruleSummary,
} from "@pp/client-core";

interface RuleCardProps {
  rule: LocalRuleView;
  index: number;
  total: number;
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
 * 单条规则卡（ADR-0003 M5.4）。
 *
 * - 左侧主体（规则名或「类型: 目标」摘要 + 类型/目标/动作/标记详情行）点击进编辑 Sheet；
 * - 动作 badge（代理/直连/拒绝语义色）；右侧启停 Switch——启用的规则在核心启动时注入；
 * - 底部栏：顺序提示 + 上移/下移（边界禁用，重排由页面层回写 sort_order）；
 * - 内置规则（builtin）：动作 badge 后追加「内置」chip；不可删除（编辑 Sheet 内无删除
 *   入口），可调整启停 / 出站 / 排序。
 */
export function RuleCard({ rule, index, total, onToggle, onMove, onEdit }: RuleCardProps) {
  const isOutbound = isOutboundAction(rule.action);
  const badgeClass = isOutbound
    ? "bg-accent/10 text-accent"
    : (ACTION_BADGE_CLASS[rule.action] ?? "bg-default-soft text-muted");
  // 指定出站动作在详情行补充目标 tag（actionLabel 仅给「指定出站」短标签）。
  const outboundTag = isOutbound ? outboundTagFromAction(rule.action) : "";
  const actionText = outboundTag ? `${actionLabel(rule.action)}: ${outboundTag}` : actionLabel(rule.action);
  // rule_set 目标为逗号分隔多 tag 时展示为 `a + b`。
  const targetText = rule.match_type === "rule_set" ? formatRuleSetTarget(rule.target) : rule.target;
  const metaDetail =
    `${matchTypeLabel(rule.match_type)}${targetText ? `: ${targetText}` : ""} · ${actionText}` +
    (rule.no_resolve ? " · no-resolve" : "") +
    (rule.invert ? " · invert" : "") +
    (rule.note ? ` · ${rule.note}` : "");

  return (
    <Card className="overflow-hidden">
      <div className="flex items-center gap-1 px-1 py-1 pl-0">
        <button type="button" onClick={onEdit} className="flex min-w-0 flex-1 flex-col gap-1 p-2.5 pl-2 text-left">
          <span className="flex min-w-0 items-center gap-2">
            <span className="min-w-0 truncate text-sm font-medium text-foreground">{ruleSummary(rule)}</span>
            <span className={`shrink-0 rounded-full px-2 py-0.5 text-xs font-medium leading-5 ${badgeClass}`}>
              {actionLabel(rule.action)}
            </span>
            {rule.builtin && (
              <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                内置
              </Chip>
            )}
          </span>
          <span className="truncate text-xs text-muted">{metaDetail}</span>
        </button>
        <Switch
          aria-label={`启用规则 ${ruleSummary(rule)}`}
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
