import { Card } from "../../components/ui";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { SwitchRow } from "./fields";

interface VpnNotificationCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * VPN 通知（移动端专属组，ADR-0003 M5.3）。
 *
 * 两个 Switch：`vpn_notify_show_traffic` / `vpn_notify_show_selection`，
 * 保存成功后追加 `notifyPrefsChanged` 热更新运行中的通知栏（失败仅 toast 警告，
 * 由 useSettingsConfig 内部处理）。
 */
export function VpnNotificationCard({ settings }: VpnNotificationCardProps) {
  return (
    <Card>
      <Card.Content className="flex flex-col gap-4">
        <SwitchRow
          label="通知栏显示流量"
          description="展示实时上传 / 下载速率"
          selected={settings.vpnNotifyTraffic}
          disabled={!settings.ready}
          onChange={(next) => void settings.onToggleVpnTraffic(next)}
        />
        <SwitchRow
          label="通知栏显示节点选择"
          description="展示当前代理分组与所选节点"
          selected={settings.vpnNotifySelection}
          disabled={!settings.ready}
          onChange={(next) => void settings.onToggleVpnSelection(next)}
        />
      </Card.Content>
    </Card>
  );
}
