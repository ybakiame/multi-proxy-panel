import { useState } from "react";
import { ArrowPathIcon } from "@heroicons/react/24/outline";
import { Button, Card, Chip, Switch } from "@heroui/react";
import type { RuleSetStatusView } from "@pp/client-core";

interface RuleSetsSectionProps {
  ruleSets: RuleSetStatusView[];
  onToggle: (communityId: string, subscribed: boolean) => Promise<boolean>;
  onUpdateNow: () => Promise<boolean>;
}

/** 订阅状态 chip：订阅中 → 品牌色；未订阅 → 灰。 */
function subscribedChip(ruleSet: RuleSetStatusView) {
  return ruleSet.subscribed ? (
    <Chip size="sm" variant="soft" color="accent" className="shrink-0">
      已订阅
    </Chip>
  ) : (
    <Chip size="sm" variant="soft" color="default" className="shrink-0">
      未订阅
    </Chip>
  );
}

function formatUpdated(lastUpdated: number): string {
  if (lastUpdated <= 0) return "从未更新";
  return new Date(lastUpdated * 1000).toLocaleString();
}

/**
 * 规则页 4 区：规则集订阅管理（ADR-0003 M5.4）。
 *
 * - 社区规则集卡片：名称 / 分类 / 订阅状态 / 缓存与最近更新时间；右侧订阅 Switch
 *   （localOverrideToggleRuleset，单飞 togglingId 防并发）；
 * - 区头「立即更新」批量拉取已订阅规则集（localOverrideUpdateRulesetsNow，busy 态）；
 * - 不做规则集市场浏览（仅已有订阅的管理，边界见任务说明）。
 */
export function RuleSetsSection({ ruleSets, onToggle, onUpdateNow }: RuleSetsSectionProps) {
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);

  const handleToggle = async (ruleSet: RuleSetStatusView, subscribed: boolean) => {
    if (togglingId !== null) return;
    setTogglingId(ruleSet.community_id);
    try {
      await onToggle(ruleSet.community_id, subscribed);
    } finally {
      setTogglingId(null);
    }
  };

  const handleUpdateNow = async () => {
    if (updating) return;
    setUpdating(true);
    try {
      await onUpdateNow();
    } finally {
      setUpdating(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">规则集</span>
          <span className="text-xs text-muted">社区规则集订阅 · 更新后供本地规则引用</span>
        </div>
        <Button
          variant="secondary"
          className="h-11 shrink-0 px-4"
          isDisabled={updating || ruleSets.length === 0}
          isPending={updating}
          onPress={() => void handleUpdateNow()}
        >
          <ArrowPathIcon className="size-4" aria-hidden="true" />
          立即更新
        </Button>
      </div>

      {ruleSets.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-muted">暂无规则集</span>
            <span className="text-xs text-muted/80">规则集市场浏览将在后续版本提供</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {ruleSets.map((ruleSet) => {
            const toggling = togglingId === ruleSet.community_id;
            return (
              <Card key={ruleSet.community_id}>
                <div className="flex items-center gap-1 px-2 py-1 pl-0">
                  <div className="flex min-w-0 flex-1 flex-col gap-0.5 px-2 py-2 pl-1">
                    <span className="flex min-w-0 items-center gap-1.5">
                      <span className="min-w-0 truncate text-sm font-medium text-foreground">
                        {ruleSet.display_name}
                      </span>
                      {subscribedChip(ruleSet)}
                    </span>
                    <span className="text-xs text-muted">{ruleSet.category}</span>
                    <span className="text-xs text-muted">
                      {ruleSet.singbox_cached ? "已缓存" : "未缓存"} · 更新于 {formatUpdated(ruleSet.last_updated)}
                    </span>
                  </div>
                  <Switch
                    aria-label={`订阅 ${ruleSet.display_name}`}
                    isSelected={ruleSet.subscribed}
                    isDisabled={toggling}
                    onChange={(next) => void handleToggle(ruleSet, next)}
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
