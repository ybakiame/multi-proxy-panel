import { BackHeader } from "../../components/BackHeader";
import { ClashApiCard } from "./ClashApiCard";
import { useSettingsConfig } from "./useSettingsConfig";

/**
 * Clash API 子页（路由 `/settings/clash-api`）。
 *
 * 迁移原设置主页 Clash API 分组：启用 Switch + 端口（1-65535 校验）+ 可选密钥。
 */
export default function ClashApiPage() {
  const settings = useSettingsConfig();

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="Clash API" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        <ClashApiCard settings={settings} />
      </div>
    </div>
  );
}
