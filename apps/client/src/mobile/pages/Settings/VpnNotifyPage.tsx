import { SubPageShell } from "../../components/SubPageShell";
import { VpnNotificationCard } from "./VpnNotificationCard";
import { useSettingsConfig } from "./useSettingsConfig";

/**
 * VPN 通知子页（路由 `/settings/vpn-notify`）。
 *
 * 迁移原设置主页 VPN 通知分组：两个 Switch（通知栏显示流量 / 节点选择），
 * 保存成功后由 `useSettingsConfig` 追加 `notifyPrefsChanged` 热更新运行中的通知栏。
 */
export default function VpnNotifyPage() {
  const settings = useSettingsConfig();

  return (
    <SubPageShell title="VPN 通知">
      <VpnNotificationCard settings={settings} />
    </SubPageShell>
  );
}
