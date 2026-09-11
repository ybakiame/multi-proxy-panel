import { BackHeader } from "../../components/BackHeader";
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
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="VPN 通知" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <VpnNotificationCard settings={settings} />
      </div>
    </div>
  );
}
