import { Card } from "@heroui/react";
import type { DnsStrategy } from "@pp/client-core";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";
import { DNS_STRATEGY_OPTIONS } from "./dnsUtils";

interface DnsRoutingCardProps {
  finalTag: string;
  strategy: DnsStrategy;
  /** 已定义 server tag 选项（来自当前草稿的 servers）。 */
  serverTagOptions: { value: string; label: string }[];
  /** takeover 且切片启用时 final 必填/引用校验的行内错误。 */
  finalTagError: string | null;
  onChangeFinalTag: (tag: string) => void;
  onChangeStrategy: (strategy: DnsStrategy) => void;
}

/**
 * DNS 页 5 区：兜底服务器（`dns.final`）与全局解析策略（`dns.strategy`）。
 *
 * final 从已定义 server tag 中选择；无服务器时禁用选择器并给出提示。
 */
export function DnsRoutingCard({
  finalTag,
  strategy,
  serverTagOptions,
  finalTagError,
  onChangeFinalTag,
  onChangeStrategy,
}: DnsRoutingCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>DNS 路由</Card.Title>
        <Card.Description>兜底服务器与全局解析策略</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium text-foreground">final 服务器</span>
          <MobileSelectSheet
            label="final 服务器"
            value={finalTag}
            onChange={onChangeFinalTag}
            placeholder={serverTagOptions.length === 0 ? "请先添加 DNS 服务器" : "请选择"}
            options={serverTagOptions}
          />
          {finalTagError ? (
            <span className="text-xs text-warning">{finalTagError}</span>
          ) : (
            <span className="text-xs text-muted">未匹配任何分流规则时使用的兜底 DNS 服务器</span>
          )}
        </div>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium text-foreground">全局解析策略</span>
          <MobileSelectSheet
            label="全局解析策略"
            value={strategy}
            onChange={(value) => onChangeStrategy(value as DnsStrategy)}
            options={DNS_STRATEGY_OPTIONS.map((option) => ({ value: option.value, label: option.label }))}
          />
        </div>
      </Card.Content>
    </Card>
  );
}
