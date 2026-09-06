import { Card, Chip } from "@heroui/react";
import { RULE_MODE_LABELS } from "./ruleModes";

interface StatusCardProps {
  /** 核心是否运行。 */
  running: boolean;
  /** 当前出站模式（`rule` / `global` / `direct`）。 */
  ruleMode: string;
}

/**
 * 首页状态卡（ADR-0003 M5）：运行状态大字（已连接/已停止，颜色区分）+ 出站模式 chip。
 *
 * 注：`ClientStatus` 无运行时长字段（见 `@pp/client-core` 的 types.ts），故不展示时长。
 */
export function StatusCard({ running, ruleMode }: StatusCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>运行状态</Card.Title>
        <Card.Description>{running ? "代理运行中，流量经系统 VPN 转发" : "代理当前未运行"}</Card.Description>
      </Card.Header>
      <Card.Content className="flex items-center justify-between gap-3">
        <span className={`text-2xl font-bold ${running ? "text-success" : "text-danger"}`}>
          {running ? "已连接" : "已停止"}
        </span>
        <div className="flex items-center gap-2">
          <span className="text-xs text-muted">出站模式</span>
          <Chip>{RULE_MODE_LABELS[ruleMode] ?? ruleMode}</Chip>
        </div>
      </Card.Content>
    </Card>
  );
}
