import { Card } from "../../components/ui";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { SwitchRow } from "./fields";

interface VpnNotificationCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * VPN 通知（移动端专属组，ADR-0003 M5.3）。
 *
 * 单个 Switch：`vpn_notify_show_selection`——通知栏展示当前生效订阅名称与所选节点
 * （分组： 节点，每 4s 经 Clash API 轮询刷新）；保存成功后追加 `notifyPrefsChanged`
 * 热更新运行中的通知栏（失败仅 toast 警告，由 useSettingsConfig 内部处理）。
 * 实时流量展示已移除（累计流量实用性低，且需额外轮询连接列表）。
 */
export function VpnNotificationCard({ settings }: VpnNotificationCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>VPN 通知</Card.Title>
        <Card.Description>Android 通知栏展示内容（移动端专属）</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <SwitchRow
          label="通知栏显示订阅与节点"
          description="展示当前生效订阅名称与所选节点（分组： 节点）"
          selected={settings.vpnNotifySelection}
          disabled={!settings.ready}
          onChange={(next) => void settings.onToggleVpnSelection(next)}
        />
        <p className="text-xs text-zinc-500 dark:text-zinc-400">
          保存后即时同步运行中的通知栏；核心未运行时将在下次启动生效。
        </p>
      </Card.Content>
    </Card>
  );
}
