import { ChevronRightIcon, PlusIcon, SignalIcon } from "@heroicons/react/24/outline";
import type { DnsServer } from "@pp/client-core";
import { delayColor } from "../../../components/proxyFormat";
import { Button, Card, Chip, Switch } from "../../../components/ui";
import { DNS_ROLE_LABELS, dnsRoleTag, dnsServerSummary, dnsServerTypeLabel } from "@pp/client-core";

/** 单个服务器的探测结果（定义收敛到 @pp/client-core `useDnsServers`，此处 re-export 保持导入路径不变）。 */
export type { DnsProbeState } from "@pp/client-core";
import type { DnsProbeState } from "@pp/client-core";

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

/**
 * DNS 管理 3 区：DNS 服务器列表。
 *
 * - 普通服务器行：启用开关（弃用保留在列表但不进运行配置，行降透明度）+
 *   名称/tag/类型/摘要 + 探测结果 chip（成功按延迟分级着色；失败显示「失败」；
 *   不支持类型显示「—」），点击行进编辑 Sheet；
 * - 内置角色服务器（`local` / `proxy`）行：不提供启用开关（内置默认 DNS 的固定组成，
 *   分流规则与 final 直接引用其 tag），点击弹「更换解析服务器」目录选择，原位替换
 *   地址、tag 不变；
 * - 区头：「探测」批量测试启用中服务器的真实查询延迟（并发），「添加服务器」进
 *   预置目录 Sheet；空态引导。
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
          <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">DNS 服务器</span>
          <span className="text-xs text-zinc-500 dark:text-zinc-400">local / proxy 为内置服务器</span>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <Button
            variant="secondary"
            className="h-11 px-3"
            isDisabled={!hasProbeable || probing}
            isPending={probing}
            onPress={onProbe}
          >
            <SignalIcon className="size-4" aria-hidden="true" />
            探测
          </Button>
          <Button variant="primary" className="h-11 px-4" onPress={onAdd}>
            <PlusIcon className="size-4" aria-hidden="true" />
            添加
          </Button>
        </div>
      </div>

      {servers.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
            <span className="text-sm text-zinc-500 dark:text-zinc-400">暂无 DNS 服务器</span>
            <span className="text-xs text-zinc-400 dark:text-zinc-500">点击「添加」从常用目录或自定义创建</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {servers.map((server) => {
            const role = dnsRoleTag(server.tag);
            const probe = probes[server.tag];
            return (
              <Card key={server.tag} className={server.enabled || role ? undefined : "opacity-55"}>
                <div className="flex items-center gap-2 p-3">
                  <button
                    type="button"
                    onClick={() => (role ? onRolePick(server) : onEdit(server))}
                    aria-label={
                      role ? `更换${DNS_ROLE_LABELS[role]}解析服务器 ${server.tag}` : `编辑服务器 ${server.tag}`
                    }
                    className="flex min-w-0 flex-1 flex-col gap-1 text-left active:opacity-80"
                  >
                    <span className="flex min-w-0 items-center gap-2">
                      <span className="min-w-0 truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
                        {server.name.trim() !== "" ? server.name.trim() : server.tag}
                      </span>
                      {role && (
                        <Chip color="default" className="shrink-0">
                          {DNS_ROLE_LABELS[role]}
                        </Chip>
                      )}
                      <span className="shrink-0 rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium leading-5 text-primary">
                        {dnsServerTypeLabel(server.server_type)}
                      </span>
                      {server.name.trim() !== "" && (
                        <span className="shrink-0 font-mono text-xs text-zinc-500 dark:text-zinc-400">
                          {server.tag}
                        </span>
                      )}
                    </span>
                    <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">
                      {dnsServerSummary(server)}
                    </span>
                  </button>
                  <ProbeChip server={server} probe={probe} />
                  {role ? (
                    <ChevronRightIcon className="size-5 shrink-0 text-zinc-500 dark:text-zinc-400" aria-hidden="true" />
                  ) : (
                    <Switch
                      aria-label={`${server.enabled ? "弃用" : "启用"}服务器 ${server.tag}`}
                      isSelected={server.enabled}
                      onValueChange={(next) => onToggle(server, next)}
                    />
                  )}
                </div>
              </Card>
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
      <Chip color="default" className="shrink-0">
        —
      </Chip>
    );
  }
  if (!probe) {
    return (
      <Chip color="default" className="shrink-0">
        未探测
      </Chip>
    );
  }
  if (probe.pending) {
    return (
      <Chip color="default" className="shrink-0">
        …
      </Chip>
    );
  }
  if (probe.unsupported) {
    return (
      <Chip color="default" className="shrink-0">
        —
      </Chip>
    );
  }
  if (probe.error !== null) {
    return (
      <Chip color="danger" className="shrink-0">
        失败
      </Chip>
    );
  }
  if (probe.latency !== null) {
    return (
      <Chip color={delayColor(probe.latency)} className="shrink-0">
        {probe.latency}ms
      </Chip>
    );
  }
  return null;
}
