import { Card } from "@heroui/react";
import type { UseSettingsConfigReturn } from "../Settings/useSettingsConfig";
import { SettingsInput } from "../Settings/fields";

interface ClashApiCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * Clash API（ADR-0005 必选切片）。
 *
 * 功能恒启用（`clash_api_enabled` 由页面层强制为 true），不提供关闭开关：
 * - `clash_api_port` 数字输入（仅 1-65535 合法才落库，非法规格展示行内错误）；
 * - `clash_api_secret` 可选密钥，留空 = 无鉴权。
 */
export function ClashApiCard({ settings }: ClashApiCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>Clash API</Card.Title>
        <Card.Description>为首页流量统计与出站模式即时切换提供本地接口</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <SettingsInput
          id="settings-clash-api-port"
          label="端口"
          value={settings.clashApiPortDraft}
          onChange={settings.onClashApiPortChange}
          placeholder="9090"
          disabled={!settings.ready}
          inputMode="numeric"
          error={settings.clashApiPortError}
        />
        <SettingsInput
          id="settings-clash-api-secret"
          label="密钥（可选）"
          value={settings.clashApiSecretDraft}
          onChange={settings.onClashApiSecretChange}
          placeholder="留空则不鉴权"
          disabled={!settings.ready}
          type="password"
          hint="留空则无鉴权"
        />
      </Card.Content>
    </Card>
  );
}
