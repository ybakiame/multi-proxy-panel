import { Card, Label, ListBox, Select, Switch } from "@heroui/react";
import type { DnsStrategy } from "@pp/client-core";
import { DNS_STRATEGY_OPTIONS } from "@pp/client-core";

interface DnsRoutingCardProps {
  finalTag: string;
  strategy: DnsStrategy;
  /** `dns.reverse_mapping`：解析后把 IP 反查回域名（供路由与连接记录）。 */
  reverseMapping: boolean;
  /** 已定义 server tag 选项（来自当前草稿的 servers）。 */
  serverTagOptions: { value: string; label: string; description?: string }[];
  /** takeover 且切片启用时 final 必填/引用校验的行内错误。 */
  finalTagError: string | null;
  onChangeFinalTag: (tag: string) => void;
  onChangeStrategy: (strategy: DnsStrategy) => void;
  onChangeReverseMapping: (value: boolean) => void;
}

/**
 * DNS 页兜底区（桌面端；语义对齐移动端 `DnsRoutingCard`）：final 服务器
 * （`dns.final`）、全局解析策略（`dns.strategy`）与反向映射开关。
 */
export function DnsRoutingCard({
  finalTag,
  strategy,
  reverseMapping,
  serverTagOptions,
  finalTagError,
  onChangeFinalTag,
  onChangeStrategy,
  onChangeReverseMapping,
}: DnsRoutingCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>DNS 路由</Card.Title>
        <Card.Description>兜底服务器与全局解析策略</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-col gap-1.5">
          <Label htmlFor="dns-final-tag">final 服务器</Label>
          <Select
            id="dns-final-tag"
            aria-label="final 服务器"
            value={finalTag}
            onChange={(key) => onChangeFinalTag(String(key ?? ""))}
            placeholder={serverTagOptions.length === 0 ? "请先添加 DNS 服务器" : "请选择"}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {serverTagOptions.map((option) => (
                  <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                    {option.label}
                    {option.description ? <span className="text-xs text-muted">（{option.description}）</span> : null}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
          {finalTagError ? (
            <span className="text-xs text-amber-500">{finalTagError}</span>
          ) : (
            <span className="text-xs text-muted">未匹配任何分流规则时使用的兜底 DNS 服务器</span>
          )}
        </div>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor="dns-strategy">全局解析策略</Label>
          <Select
            id="dns-strategy"
            aria-label="全局解析策略"
            value={strategy}
            onChange={(key) => onChangeStrategy(String(key) as DnsStrategy)}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {DNS_STRATEGY_OPTIONS.map((option) => (
                  <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                    {option.label}
                    <ListBox.ItemIndicator />
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
        </div>

        <div className="flex items-center justify-between gap-3">
          <div className="flex flex-col gap-0.5">
            <span className="text-sm text-foreground">反向映射（reverse_mapping）</span>
            <span className="text-xs text-muted">解析后把 IP 反查回域名，供路由规则与连接记录使用</span>
          </div>
          <Switch aria-label="反向映射" isSelected={reverseMapping} onChange={onChangeReverseMapping}>
            <Switch.Content>
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
            </Switch.Content>
          </Switch>
        </div>
      </Card.Content>
    </Card>
  );
}
