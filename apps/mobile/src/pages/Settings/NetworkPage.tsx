import { BackHeader } from "../../components/BackHeader";
import { NetworkCard } from "./NetworkCard";
import { useSettingsConfig } from "./useSettingsConfig";

/**
 * 网络子页（路由 `/settings/network`）。
 *
 * 迁移原设置主页网络分组：本地混合端口（`mixed_port`，1-65535 校验）。
 */
export default function NetworkPage() {
  const settings = useSettingsConfig();

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="网络" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <NetworkCard settings={settings} />
      </div>
    </div>
  );
}
