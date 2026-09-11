import { Alert } from "@heroui/react";
import { BackHeader } from "../../components/BackHeader";
import { useSettingsConfig } from "../Settings/useSettingsConfig";
import { NetworkCard } from "./NetworkCard";

/**
 * 网络子页（路由 `/config/network`，ADR-0005 必选切片）。
 *
 * TUN 入站为代理基础能力（Android 由 `panel_features_tun_enabled` 恒注入），始终启用、
 * 无切片级关闭开关；此处仅调整本地混合端口（`mixed_port`，1-65535 校验）。
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
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>始终启用</Alert.Title>
            <Alert.Description>TUN 入站为代理基础能力，始终启用；此处仅调整本地混合端口。</Alert.Description>
          </Alert.Content>
        </Alert>
        <NetworkCard settings={settings} />
      </div>
    </div>
  );
}
