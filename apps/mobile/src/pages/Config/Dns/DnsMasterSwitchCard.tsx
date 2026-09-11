import { Card, Switch } from "@heroui/react";

interface DnsMasterSwitchCardProps {
  /** `dns.enabled`：DNS 切片是否注入运行配置。 */
  enabled: boolean;
  onToggle: (next: boolean) => void;
}

/**
 * DNS 页 1 区：切片总开关卡（对齐 `MasterSwitchCard` 风格）。
 *
 * 关闭后 DNS 切片完全不注入运行配置（正文仍可编辑，保存时不生效）。
 */
export function DnsMasterSwitchCard({ enabled, onToggle }: DnsMasterSwitchCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>DNS 切片总开关</Card.Title>
        <Card.Description>
          <div className="flex w-full items-center justify-between gap-3">
            <span>关闭后 DNS 切片不会注入运行配置</span>
            <Switch
              aria-label="启用 DNS 切片"
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
