import { Card } from "@heroui/react";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { SettingsInput, SwitchRow } from "./fields";

interface ClashApiCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * Clash API（ADR-0003 M5.3）。
 *
 * - `clash_api_enabled` Switch：开启后首页可显示流量统计、出站模式可即时切换；
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
        <SwitchRow
          label="启用 Clash API"
          description="开启后首页可显示流量统计、出站模式可即时切换"
          selected={settings.clashApiEnabled}
          disabled={!settings.ready}
          onChange={(next) => void settings.onToggleClashApi(next)}
        />
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
