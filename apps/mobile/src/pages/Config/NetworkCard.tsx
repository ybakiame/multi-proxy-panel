import { Card } from "@heroui/react";
import type { UseSettingsConfigReturn } from "../Settings/useSettingsConfig";
import { SettingsInput, SwitchRow } from "../Settings/fields";

interface NetworkCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * 网络（ADR-0003 M5.3）：本地混合端口 `mixed_port` + IPv6 参数开关 `ipv6_enabled`。
 *
 * 数字输入仅 1-65535 合法才落库（非法规格展示行内错误，不保存）。
 * IPv6 为参数开关（非功能开关），关闭时 DNS 不返回 AAAA 记录。
 */
export function NetworkCard({ settings }: NetworkCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>网络</Card.Title>
        <Card.Description>本地核心监听配置</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <SettingsInput
          id="settings-mixed-port"
          label="本地混合端口"
          value={settings.mixedPortDraft}
          onChange={settings.onMixedPortChange}
          placeholder="1080"
          disabled={!settings.ready}
          inputMode="numeric"
          error={settings.mixedPortError}
          hint="其他 App 手动配置代理（HTTP / SOCKS）时使用的本地端口"
        />
        <SwitchRow
          label="IPv6"
          description="关闭时 DNS 不返回 AAAA 记录（推荐，节点不支持 IPv6 时最稳定）；开启后双栈，IPv6 流量经隧道按规则分流"
          selected={settings.ipv6Enabled}
          disabled={!settings.ready}
          onChange={(next) => void settings.onToggleIpv6(next)}
        />
      </Card.Content>
    </Card>
  );
}
