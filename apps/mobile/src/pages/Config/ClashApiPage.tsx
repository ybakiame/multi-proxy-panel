import { useEffect, useRef } from "react";
import { Alert } from "@heroui/react";
import { useClientConfig, useSaveConfig } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { useSettingsConfig } from "../Settings/useSettingsConfig";
import { ClashApiCard } from "./ClashApiCard";

/**
 * Clash API 子页（路由 `/config/clash-api`，ADR-0005 必选切片）。
 *
 * Clash API 为仪表盘与节点页数据源，始终启用；页面无关闭开关，仅调整端口与密钥。
 *
 * 存量 `clash_api_enabled: false`（历史关闭开关）在进入本页时静默纠正为 true 并落盘；
 * 纠正后缓存/磁盘基底均为 true，后续端口/密钥保存经共享 `useSettingsConfig` 叠加在该
 * 基底上，恒写 true。共享 hook 的通用逻辑保持不变。
 */
export default function ClashApiPage() {
  const settings = useSettingsConfig();
  const { data: config } = useClientConfig();
  const { mutateAsync: saveConfigAsync } = useSaveConfig();

  // 必选切片恒启用：仅本页负责纠正存量关闭态（一次性，失败下次进入重试）。
  const correctedRef = useRef(false);
  useEffect(() => {
    if (correctedRef.current || !config || config.clash_api_enabled) {
      return;
    }
    correctedRef.current = true;
    void saveConfigAsync({ ...config, clash_api_enabled: true }).catch(() => {
      // 静默纠正失败：保留原值不阻塞页面，重置标记以便下次进入重试。
      correctedRef.current = false;
    });
  }, [config, saveConfigAsync]);

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
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>始终启用</Alert.Title>
            <Alert.Description>Clash API 为仪表盘与节点页数据源，始终启用，此处仅调整参数。</Alert.Description>
          </Alert.Content>
        </Alert>
        <ClashApiCard settings={settings} />
      </div>
    </div>
  );
}
