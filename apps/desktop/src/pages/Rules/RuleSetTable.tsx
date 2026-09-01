import { Button, Switch } from "@heroui/react";
import type { RuleSetStatusView } from "../../api";

export interface RuleSetTableProps {
  ruleSets: RuleSetStatusView[];
  onToggle: (communityId: string, subscribed: boolean) => void;
  onUpdateNow: () => void;
  busy: boolean;
}

export function RuleSetTable({ ruleSets, onToggle, onUpdateNow, busy }: RuleSetTableProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between">
        <span className="text-sm font-medium">规则集订阅</span>
        <Button size="sm" variant="secondary" isPending={busy} onPress={onUpdateNow}>
          立即更新
        </Button>
      </div>
      {ruleSets.length === 0 ? (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          暂无规则集
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {ruleSets.map((rs) => (
            <div
              key={rs.community_id}
              className="flex items-center gap-3 rounded-lg border border-border/60 bg-surface-secondary/40 p-3"
            >
              <div className="flex min-w-0 flex-1 flex-col gap-0.5">
                <span className="text-sm font-medium text-foreground">{rs.display_name}</span>
                <span className="text-xs text-muted">
                  {rs.category} · {rs.subscribed ? "已订阅" : "未订阅"}
                  {rs.last_updated > 0 ? ` · 更新于 ${new Date(rs.last_updated * 1000).toLocaleString()}` : ""}
                </span>
                <span className="text-xs text-muted">
                  sing-box: {rs.singbox_cached ? "已缓存" : "未缓存"} · mihomo: {rs.mihomo_cached ? "已缓存" : "未缓存"}
                </span>
              </div>
              <Switch
                size="sm"
                isSelected={rs.subscribed}
                onChange={() => onToggle(rs.community_id, !rs.subscribed)}
                aria-label={`订阅 ${rs.display_name}`}
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
