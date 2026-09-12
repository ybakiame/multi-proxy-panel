import { Card } from "@heroui/react";
import type { DnsStrategy } from "@pp/client-core";
import { MobileSelectSheet, type MobileSelectOption } from "../../../components/MobileSelectSheet";
import { ROUTE_STRATEGY_OPTIONS } from "./routeUtils";

interface RouteSettingsCardProps {
  /** `route.final`：默认出站 tag（空串 = 模板默认 `proxy`）。 */
  finalTag: string;
  /** `route.default_domain_resolver.server`（空串 = 不覆写）。 */
  resolverServer: string;
  /** `route.default_domain_resolver.strategy`（`null` = 跟随全局）。 */
  resolverStrategy: DnsStrategy | null;
  finalTagOptions: MobileSelectOption[];
  resolverServerOptions: MobileSelectOption[];
  finalTagError: string | null;
  resolverServerError: string | null;
  onChangeFinalTag: (tag: string) => void;
  onChangeResolverServer: (server: string) => void;
  onChangeResolverStrategy: (strategy: DnsStrategy | null) => void;
}

/**
 * 路由页 2 区：路由设置（`route.final` 与 `route.default_domain_resolver`）。
 *
 * final 从内置出站 / 启用切片出站 / 订阅节点中选择，空串表示沿用模板默认 `proxy`；
 * resolver 的 server 从内置 DNS server / DNS 切片 server 中选择，strategy 可置空
 * 跟随全局 DNS 策略。
 */
export function RouteSettingsCard({
  finalTag,
  resolverServer,
  resolverStrategy,
  finalTagOptions,
  resolverServerOptions,
  finalTagError,
  resolverServerError,
  onChangeFinalTag,
  onChangeResolverServer,
  onChangeResolverStrategy,
}: RouteSettingsCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>路由设置</Card.Title>
        <Card.Description>默认出站与出站域名解析器</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium text-foreground">默认出站（final）</span>
          <MobileSelectSheet label="默认出站" value={finalTag} onChange={onChangeFinalTag} options={finalTagOptions} />
          {finalTagError ? (
            <span className="text-xs text-warning">{finalTagError}</span>
          ) : (
            <span className="text-xs text-muted">未匹配任何规则时使用的兜底出站</span>
          )}
        </div>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium text-foreground">默认域名解析服务器</span>
          <MobileSelectSheet
            label="默认域名解析服务器"
            value={resolverServer}
            onChange={onChangeResolverServer}
            options={resolverServerOptions}
          />
          {resolverServerError ? (
            <span className="text-xs text-warning">{resolverServerError}</span>
          ) : (
            <span className="text-xs text-muted">出站域名使用的 DNS 服务器（default_domain_resolver）</span>
          )}
        </div>

        <div className="flex flex-col gap-1.5">
          <span className="text-sm font-medium text-foreground">解析策略</span>
          <MobileSelectSheet
            label="解析策略"
            value={resolverStrategy ?? ""}
            onChange={(value) => onChangeResolverStrategy(value === "" ? null : (value as DnsStrategy))}
            options={ROUTE_STRATEGY_OPTIONS}
          />
          <span className="text-xs text-muted">出站域名解析策略，留空则跟随全局 DNS 策略</span>
        </div>
      </Card.Content>
    </Card>
  );
}
