import { Button, Card, Chip } from "@heroui/react";
import type { ClientConfig, ClientStatus, CoreType, SubscriptionView } from "../api";

const CORE_LABELS: Record<CoreType, string> = {
  singbox: "sing-box",
  mihomo: "mihomo",
};

/** 核心类型展示名（兼容 `singbox` / `mihomo` 小写 serde 值）。 */
export function coreLabel(value: string): string {
  return CORE_LABELS[(value === "SingBox" ? "singbox" : value === "Mihomo" ? "mihomo" : value) as CoreType] ?? value;
}

interface DashboardStatusCardsProps {
  config: ClientConfig | null | undefined;
  status: ClientStatus | null | undefined;
  isAndroid: boolean;
  running: boolean;
  /** 当前生效订阅（展示节点数）。 */
  activeSub: SubscriptionView | null;
  linkCopied: boolean;
  onCopyLink: () => void;
  onOpenPanel: () => void;
}

/** 仪表盘「C. 状态卡片」区块：核心状态 / 节点数 / 规则数 / MITM / 系统代理 / Clash 面板。 */
export default function DashboardStatusCards({
  config,
  status,
  isAndroid,
  running,
  activeSub,
  linkCopied,
  onCopyLink,
  onOpenPanel,
}: DashboardStatusCardsProps) {
  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
      <Card>
        <Card.Header>
          <Card.Title>核心状态</Card.Title>
          <Card.Description>
            {isAndroid
              ? `当前运行核心：${coreLabel(config?.core_type ?? "singbox")}`
              : config?.core_type === "mihomo"
                ? "mihomo"
                : "sing-box"}
            {config ? ` · 混合端口 ${config.mixed_port}` : ""}
          </Card.Description>
        </Card.Header>
        <Card.Content>
          {running ? <Chip color="success">运行中</Chip> : <Chip color="danger">已停止</Chip>}
        </Card.Content>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>节点数量</Card.Title>
          <Card.Description>当前生效订阅的可用节点数</Card.Description>
        </Card.Header>
        <Card.Content>
          <span className="text-sm">{activeSub ? activeSub.node_count : "-"}</span>
        </Card.Content>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>规则数量</Card.Title>
          <Card.Description>本次合成配置的规则条数</Card.Description>
        </Card.Header>
        <Card.Content>
          <span className="text-sm">{status?.rule_count ?? 0}</span>
        </Card.Content>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>MITM 地址</Card.Title>
          <Card.Description>中间人代理监听地址</Card.Description>
        </Card.Header>
        <Card.Content>
          <span className="text-sm">{status?.mitm_addr ?? "未启用"}</span>
        </Card.Content>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>系统代理</Card.Title>
          <Card.Description>是否已接管系统代理</Card.Description>
        </Card.Header>
        <Card.Content>
          {status?.system_proxy ? <Chip color="success">已启用</Chip> : <Chip color="danger">未启用</Chip>}
        </Card.Content>
      </Card>

      <Card>
        <Card.Header>
          <Card.Title>Clash 面板</Card.Title>
          <Card.Description>面板 API 开启状态与访问入口</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-3">
          {!config?.clash_api_enabled ? (
            <div className="flex flex-col gap-1">
              <span className="text-sm">未启用</span>
              <span className="text-xs text-muted">在「设置 → Clash 面板」开启</span>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-2">
                {status?.clash_api_url && running ? (
                  <Chip color="success">运行中</Chip>
                ) : (
                  <Chip color="danger">未运行</Chip>
                )}
              </div>
              <span className="break-all font-mono text-xs text-muted">
                http://127.0.0.1:{config.clash_api_port}/ui
              </span>
              <div className="flex items-center gap-2">
                <Button size="sm" variant="secondary" onPress={onCopyLink}>
                  {linkCopied ? "已复制" : "复制链接"}
                </Button>
                <Button size="sm" variant="secondary" onPress={onOpenPanel}>
                  打开面板
                </Button>
              </div>
              <span className="text-xs text-muted">
                首次打开会自动下载面板资源，需网络可达；空白时检查网络或稍候重试
              </span>
            </>
          )}
        </Card.Content>
      </Card>
    </div>
  );
}
