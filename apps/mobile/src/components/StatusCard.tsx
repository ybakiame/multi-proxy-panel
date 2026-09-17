import { Card, Chip } from "@heroui/react";
import { RULE_MODE_LABELS } from "./ruleModes";
import { RuleModeSwitch } from "./RuleModeSwitch";

interface StatusCardProps {
  /** 核心是否运行。 */
  running: boolean;
  /** 当前出站模式（`rule` / `global` / `direct`）。 */
  ruleMode: string;
  /** Clash API 是否开启（决定运行期出站模式「即时生效」提示）。 */
  clashApiEnabled: boolean;
  /** 未就绪的内置规则集（非空 = 降级运行，后台重试补齐后自动重载）。 */
  missingRuleSets?: string[];
}

/**
 * 首页状态卡：运行状态大字（已连接/已停止，颜色区分）+ 出站模式。
 *
 * 出站模式在两种状态下都有「出站模式」标签行（此前运行中为无标签分段控件、
 * 停止时为右侧 chip，语义不统一）：
 * - 运行中：标签行 + 分段控件（`RuleModeSwitch`，切换即时生效）；
 * - 未运行：标签行 + 已保存模式 chip（分段隐藏，与旧行为一致）。
 *
 * 注：`ClientStatus` 无运行时长字段（见 `@pp/client-core` 的 types.ts），故不展示时长。
 */
export function StatusCard({ running, ruleMode, clashApiEnabled, missingRuleSets = [] }: StatusCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>运行状态</Card.Title>
        <Card.Description>{running ? "代理运行中，流量经系统 VPN 转发" : "代理当前未运行"}</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <span className={`text-2xl font-bold ${running ? "text-success" : "text-danger"}`}>
          {running ? "已连接" : "已停止"}
        </span>
        {running ? (
          <RuleModeSwitch value={ruleMode} running={running} clashApiEnabled={clashApiEnabled} />
        ) : (
          <div className="flex min-h-11 items-center justify-between gap-3">
            <span className="text-sm font-medium text-foreground">出站模式</span>
            <Chip>{RULE_MODE_LABELS[ruleMode] ?? ruleMode}</Chip>
          </div>
        )}
        {running && missingRuleSets.length > 0 && (
          <span className="text-xs text-warning">
            分流规则集未全部就绪（{missingRuleSets.join("、")}），已降级运行；后台重试补齐后将自动恢复完整分流
          </span>
        )}
      </Card.Content>
    </Card>
  );
}
