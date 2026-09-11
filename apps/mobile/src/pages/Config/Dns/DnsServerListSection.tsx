import { PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card } from "@heroui/react";
import type { DnsServer } from "@pp/client-core";
import { dnsServerSummary, dnsServerTypeLabel } from "./dnsUtils";

interface DnsServerListSectionProps {
  servers: DnsServer[];
  onEdit: (server: DnsServer) => void;
  onAdd: () => void;
}

/**
 * DNS 页 3 区：DNS 服务器列表（ADR-0005 P0-4b）。
 *
 * 区头「添加服务器」按钮进编辑 Sheet；卡片展示 tag / 类型 / 地址 / 出站摘要，
 * 点击进编辑 Sheet（删除入口在 Sheet 内）。无服务器时展示空态引导。
 */
export function DnsServerListSection({ servers, onEdit, onAdd }: DnsServerListSectionProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">DNS 服务器</span>
          <span className="text-xs text-muted">定义上游 DNS，供分流规则与 final 引用</span>
        </div>
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加服务器
        </Button>
      </div>

      {servers.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-muted">暂无 DNS 服务器</span>
            <span className="text-xs text-muted/80">点击「添加服务器」创建，例如 1.1.1.1 或本地 DNS</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {servers.map((server) => (
            <Card key={server.tag}>
              <button
                type="button"
                onClick={() => onEdit(server)}
                aria-label={`编辑服务器 ${server.tag}`}
                className="flex min-w-0 flex-col gap-1 p-3 text-left active:opacity-80"
              >
                <span className="flex min-w-0 items-center gap-2">
                  <span className="min-w-0 truncate text-sm font-medium text-foreground">{server.tag}</span>
                  <span className="shrink-0 rounded-full bg-accent/10 px-2 py-0.5 text-xs font-medium leading-5 text-accent">
                    {dnsServerTypeLabel(server.server_type)}
                  </span>
                </span>
                <span className="truncate text-xs text-muted">{dnsServerSummary(server)}</span>
              </button>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
