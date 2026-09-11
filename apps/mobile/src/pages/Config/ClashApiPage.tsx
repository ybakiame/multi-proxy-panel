import { Alert } from "@heroui/react";
import { BackHeader } from "../../components/BackHeader";
import { useSettingsConfig } from "../Settings/useSettingsConfig";
import { ClashApiCard } from "./ClashApiCard";

/**
 * Clash API 子页（路由 `/config/clash-api`，ADR-0005 必选切片）。
 *
 * Clash API 为仪表盘与节点页数据源，属应用核心能力；此处仅调整端口与密钥。
 * 现有 `clash_api_enabled` 开关为既有客户端能力开关（控制本地 API 端点），按现状保留，
 * 说明文案避免与开关语义冲突（不声明「始终启用」）。
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
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>应用核心能力</Alert.Title>
            <Alert.Description>
              Clash API 为仪表盘与节点页数据源，属应用核心能力；建议保持启用，此处仅调整端口与密钥。
            </Alert.Description>
          </Alert.Content>
        </Alert>
        <ClashApiCard settings={settings} />
      </div>
    </div>
  );
}
