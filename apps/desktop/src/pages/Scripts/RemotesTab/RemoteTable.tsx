/**
 * Table component displaying remote resources.
 */

import { Avatar, Button, Switch, Table } from "@heroui/react";
import type { RemoteResource } from "../../../api";
import { formatInterval, normalizeDialect } from "../utils";

interface RemoteTableProps {
  remotes: RemoteResource[];
  iconCache: Record<string, string>;
  busy: boolean;
  onToggle: (remote: RemoteResource) => void;
  onEdit: (remote: RemoteResource) => void;
  onRemove: (name: string) => void;
}

export default function RemoteTable({ remotes, iconCache, busy, onToggle, onEdit, onRemove }: RemoteTableProps) {
  if (remotes.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
        <span className="text-sm text-muted">暂无远程资源</span>
        <span className="text-xs text-muted/80">点击「添加资源」创建第一条订阅</span>
      </div>
    );
  }

  return (
    <Table>
      <Table.ScrollContainer>
        <Table.Content aria-label="远程资源" className="min-w-[720px]">
          <Table.Header>
            <Table.Column>图标</Table.Column>
            <Table.Column isRowHeader>名称</Table.Column>
            <Table.Column>描述</Table.Column>
            <Table.Column>类型</Table.Column>
            <Table.Column>更新间隔</Table.Column>
            <Table.Column>启用</Table.Column>
            <Table.Column>操作</Table.Column>
          </Table.Header>
          <Table.Body>
            {remotes.map((remote) => (
              <Table.Row key={remote.name}>
                <Table.Cell>
                  <Avatar size="sm" className="h-6 w-6">
                    {remote.icon ? (
                      <Avatar.Image src={iconCache[remote.name] ?? remote.icon} alt={`${remote.name} 图标`} />
                    ) : null}
                    <Avatar.Fallback color="accent">{(remote.name.charAt(0) || "?").toUpperCase()}</Avatar.Fallback>
                  </Avatar>
                </Table.Cell>
                <Table.Cell className="max-w-[180px] truncate">
                  <span title={remote.name}>{remote.name}</span>
                </Table.Cell>
                <Table.Cell className="max-w-[200px] truncate">
                  <span title={remote.description ?? "-"}>{remote.description ?? "-"}</span>
                </Table.Cell>
                <Table.Cell>
                  {remote.kind === "Script" ? "脚本" : `片段 / ${normalizeDialect(remote.dialect) ?? remote.dialect}`}
                </Table.Cell>
                <Table.Cell>{formatInterval(remote.update_interval_secs)}</Table.Cell>
                <Table.Cell>
                  <Switch
                    aria-label={`启用 ${remote.name}`}
                    isSelected={remote.enabled}
                    onChange={() => void onToggle(remote)}
                  >
                    <Switch.Content>
                      <Switch.Control>
                        <Switch.Thumb />
                      </Switch.Control>
                      <span className="sr-only">{remote.enabled ? "启用" : "停用"}</span>
                    </Switch.Content>
                  </Switch>
                </Table.Cell>
                <Table.Cell>
                  <div className="flex items-center gap-2">
                    <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => onEdit(remote)}>
                      编辑
                    </Button>
                    <Button size="sm" variant="tertiary" isDisabled={busy} onPress={() => void onRemove(remote.name)}>
                      删除
                    </Button>
                  </div>
                </Table.Cell>
              </Table.Row>
            ))}
          </Table.Body>
        </Table.Content>
      </Table.ScrollContainer>
    </Table>
  );
}
