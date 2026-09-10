import { Card, Switch } from "@heroui/react";

interface MasterSwitchCardProps {
  /** `singbox.enabled`：本地规则 Override 是否注入运行配置。 */
  enabled: boolean;
  onToggle: (next: boolean) => void;
}

/**
 * 规则页 1 区：本地规则总开关卡（ADR-0003 M5.4）。
 *
 * 关闭后本地规则与规则集均不注入运行配置（说明文案与切换入口同卡）。
 */
export function MasterSwitchCard({ enabled, onToggle }: MasterSwitchCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>规则总开关</Card.Title>
        <Card.Description>
          <div className="flex items-center justify-between gap-3 w-full">
            <text>控制规则是否允许注入运行配置</text>
            <Switch
              aria-label="启用本地规则"
              isSelected={enabled}
              onChange={(next) => onToggle(next)}
              className="shrink-0"
            >
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
              </Switch.Content>
            </Switch>
          </div>
        </Card.Description>
      </Card.Header>
    </Card>
  );
}
