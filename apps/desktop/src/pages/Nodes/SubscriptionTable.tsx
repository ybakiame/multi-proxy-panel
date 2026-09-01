import { Button, Chip, Meter, Switch, Table } from "@heroui/react";
import type { ProfileView, SubscriptionView } from "../../api";
import { formatLabel, formatColor, usageText, usagePercent, usageColor, formatExpire } from "./utils";

interface SubscriptionTableProps {
  subs: SubscriptionView[];
  profiles: ProfileView[];
  busy: boolean;
  refreshingId: string | null;
  onToggle: (sub: SubscriptionView) => void;
  onRefresh: (id: string) => void;
  onRemove: (id: string) => void;
  onEdit: (sub: SubscriptionView) => void;
  onPreview: (sub: SubscriptionView) => void;
}

export function SubscriptionTable({
  subs,
  profiles,
  busy,
  refreshingId,
  onToggle,
  onRefresh,
  onRemove,
  onEdit,
  onPreview,
}: SubscriptionTableProps) {
  if (subs.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
        <span className="text-sm text-muted">暂无订阅</span>
        <span className="text-xs text-muted/80">点击「添加订阅」添加首个订阅源，启用后重启代理应用生效</span>
      </div>
    );
  }

  return (
    <Table>
      <Table.ScrollContainer>
        <Table.Content aria-label="订阅列表" className="min-w-[880px]">
          <Table.Header>
            <Table.Column isRowHeader>名称</Table.Column>
            <Table.Column>URL</Table.Column>
            <Table.Column>节点数</Table.Column>
            <Table.Column>格式</Table.Column>
            <Table.Column>用量</Table.Column>
            <Table.Column>到期时间</Table.Column>
            <Table.Column>覆写</Table.Column>
            <Table.Column>启用</Table.Column>
            <Table.Column>操作</Table.Column>
          </Table.Header>
          <Table.Body>
            {subs.map((sub) => {
              const percent = usagePercent(sub.userinfo);
              const linkedProfile = sub.profile_id ? profiles.find((p) => p.id === sub.profile_id) : undefined;
              return (
                <Table.Row key={sub.id}>
                  <Table.Cell className="max-w-[200px] truncate">
                    <span title={sub.name}>{sub.name}</span>
                  </Table.Cell>
                  <Table.Cell className="max-w-[240px] truncate font-mono text-xs">
                    <span title={sub.url}>{sub.url}</span>
                  </Table.Cell>
                  <Table.Cell>{sub.node_count > 0 ? sub.node_count : "-"}</Table.Cell>
                  <Table.Cell>
                    {sub.format ? (
                      <Chip size="sm" variant="soft" color={formatColor(sub.format)}>
                        {formatLabel(sub.format)}
                      </Chip>
                    ) : (
                      <span className="text-muted">-</span>
                    )}
                  </Table.Cell>
                  <Table.Cell className="min-w-[180px]">
                    <Meter
                      aria-label={`${sub.name} 流量用量`}
                      value={percent}
                      size="sm"
                      color={usageColor(percent)}
                      valueLabel={usageText(sub.userinfo)}
                      className="w-full"
                    >
                      <Meter.Output />
                      <Meter.Track>
                        <Meter.Fill />
                      </Meter.Track>
                    </Meter>
                  </Table.Cell>
                  <Table.Cell>{formatExpire(sub.userinfo?.expire)}</Table.Cell>
                  <Table.Cell>
                    {sub.profile_id ? (
                      linkedProfile ? (
                        <span className="block max-w-[140px] truncate text-xs" title={linkedProfile.name}>
                          {linkedProfile.name}
                        </span>
                      ) : (
                        <span className="text-xs text-warning">已失效</span>
                      )
                    ) : (
                      <span className="text-muted">-</span>
                    )}
                  </Table.Cell>
                  <Table.Cell>
                    <Switch
                      aria-label={`启用 ${sub.name}`}
                      isSelected={sub.enabled}
                      isDisabled={busy}
                      onChange={() => void onToggle(sub)}
                    >
                      <Switch.Content>
                        <Switch.Control>
                          <Switch.Thumb />
                        </Switch.Control>
                        <span className="sr-only">{sub.enabled ? "启用" : "停用"}</span>
                      </Switch.Content>
                    </Switch>
                  </Table.Cell>
                  <Table.Cell>
                    <div className="flex items-center gap-2">
                      <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => onPreview(sub)}>
                        预览
                      </Button>
                      <Button size="sm" variant="secondary" isDisabled={busy} onPress={() => onEdit(sub)}>
                        编辑
                      </Button>
                      <Button
                        size="sm"
                        variant="secondary"
                        isDisabled={busy}
                        isPending={refreshingId === sub.id}
                        onPress={() => void onRefresh(sub.id)}
                      >
                        刷新
                      </Button>
                      <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => void onRemove(sub.id)}>
                        删除
                      </Button>
                    </div>
                  </Table.Cell>
                </Table.Row>
              );
            })}
          </Table.Body>
        </Table.Content>
      </Table.ScrollContainer>
    </Table>
  );
}
