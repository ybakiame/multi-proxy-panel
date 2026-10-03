import { Button, Chip, Switch } from "@heroui/react";
import { ChevronRightIcon, PlusIcon, SignalIcon } from "@heroicons/react/24/outline";
import type { DnsProbeState, DnsServer } from "@pp/client-core";
import { DNS_ROLE_LABELS, dnsRoleTag, dnsServerSummary, dnsServerTypeLabel } from "@pp/client-core";

interface DnsServerListSectionProps {
  servers: DnsServer[];
  /** tag → 探测结果（无键 = 未探测）。 */
  probes: Readonly<Record<string, DnsProbeState>>;
  /** 批量探测进行中（按钮置 pending）。 */
  probing: boolean;
  onToggle: (server: DnsServer, enabled: boolean) => void;
  onProbe: () => void;
  onEdit: (server: DnsServer) => void;
  onAdd: () => void;
  /** 内置角色服务器（local / proxy）点击更换：弹目录选择而非编辑表单。 */
  onRolePick: (server: DnsServer) => void;
}

/** 可探测类型（与 Rust `dns_probe` 支持范围一致）。 */
const PROBEABLE_TYPES = new Set(["udp", "tls", "https"]);

/** 延迟分级着色（与 Proxies 页一致：绿 < 300ms，黄 < 800ms）。 */
function delayColor(delay: number): "success" | "warning" | "default" {
  if (delay < 300) return "success";
  if (delay < 800) return "warning";
  return "default";
}

/**
 * DNS 页服务器列表区（桌面端；语义对齐移动端 `DnsServerListSection`）。
 *
 * - 普通服务器行：启用开关（弃用保留在列表但不进运行配置，行降透明度）+
 *   名称/tag/类型/摘要 + 探测结果 chip，点击行进编辑弹窗；
 * - 内置角色服务器（`local` / `proxy`）行：不提供启用开关，点击弹「更换解析服务器」
 *   目录选择，原位替换地址、tag 不变；
 * - 区头：「探测」批量测试启用中服务器的真实查询延迟（并发），「添加服务器」进
 *   预置目录弹窗；空态引导。
 */
export function DnsServerListSection({
  servers,
  probes,
  probing,
  onToggle,
  onProbe,
  onEdit,
  onAdd,
  onRolePick,
}: DnsServerListSectionProps) {
  const hasProbeable = servers.some((server) => server.enabled && PROBEABLE_TYPES.has(server.server_type));

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium text-foreground">DNS 服务器</span>
          <span className="text-xs text-muted">local / proxy 为内置服务器</span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            size="sm"
            variant="secondary"
            isDisabled={!hasProbeable || probing}
            isPending={probing}
            onPress={onProbe}
          >
            <SignalIcon className="size-4" aria-hidden="true" />
            探测
          </Button>
          <Button size="sm" variant="primary" onPress={onAdd}>
            <PlusIcon className="size-4" aria-hidden="true" />
            添加
          </Button>
        </div>
      </div>

      {servers.length === 0 ? (
        <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
          暂无 DNS 服务器，点击「添加」从常用目录或自定义创建
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          {servers.map((server) => {
            const role = dnsRoleTag(server.tag);
            const probe = probes[server.tag];
            return (
              <div
                key={server.tag}
                className={`flex items-center gap-2 rounded-lg border border-border/60 bg-surface-secondary/40 p-3 ${
                  server.enabled || role ? "" : "opacity-55"
                }`}
              >
                <button
                  type="button"
                  onClick={() => (role ? onRolePick(server) : onEdit(server))}
                  aria-label={
                    role ? `更换${DNS_ROLE_LABELS[role]}解析服务器 ${server.tag}` : `编辑服务器 ${server.tag}`
                  }
                  className="flex min-w-0 flex-1 flex-col gap-1 text-left"
                >
                  <span className="flex min-w-0 items-center gap-2">
                    <span className="min-w-0 truncate text-sm font-medium text-foreground">
                      {server.name.trim() !== "" ? server.name.trim() : server.tag}
                    </span>
                    {role && (
                      <Chip size="sm" variant="soft" color="default" className="shrink-0">
                        {DNS_ROLE_LABELS[role]}
                      </Chip>
                    )}
                    <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                      {dnsServerTypeLabel(server.server_type)}
                    </Chip>
                    {server.name.trim() !== "" && (
                      <span className="shrink-0 font-mono text-xs text-muted">{server.tag}</span>
                    )}
                  </span>
                  <span className="truncate text-xs text-muted">{dnsServerSummary(server)}</span>
                </button>
                <ProbeChip server={server} probe={probe} />
                {role ? (
                  <ChevronRightIcon className="size-5 shrink-0 text-muted" aria-hidden="true" />
                ) : (
                  <Switch
                    size="sm"
                    aria-label={`${server.enabled ? "弃用" : "启用"}服务器 ${server.tag}`}
                    isSelected={server.enabled}
                    onChange={(next) => onToggle(server, next)}
                  >
                    <Switch.Content>
                      <Switch.Control>
                        <Switch.Thumb />
                      </Switch.Control>
                    </Switch.Content>
                  </Switch>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** 探测结果 chip：未探测标注；探测中省略号；成功按延迟分级；失败/不支持明确标注。 */
function ProbeChip({ server, probe }: { server: DnsServer; probe?: DnsProbeState }) {
  if (!PROBEABLE_TYPES.has(server.server_type)) {
    return (
      <Chip size="sm" variant="soft" color="default" className="shrink-0">
        —
      </Chip>
    );
  }
  if (!probe) {
    return (
      <Chip size="sm" variant="soft" color="default" className="shrink-0">
        未探测
      </Chip>
    );
  }
  if (probe.pending) {
    return (
      <Chip size="sm" variant="soft" color="default" className="shrink-0">
        …
      </Chip>
    );
  }
  if (probe.unsupported) {
    return (
      <Chip size="sm" variant="soft" color="default" className="shrink-0">
        —
      </Chip>
    );
  }
  if (probe.error !== null) {
    return (
      <Chip size="sm" variant="soft" color="danger" className="shrink-0">
        失败
      </Chip>
    );
  }
  if (probe.latency !== null) {
    return (
      <Chip size="sm" variant="soft" color={delayColor(probe.latency)} className="shrink-0">
        {probe.latency}ms
      </Chip>
    );
  }
  return null;
}
