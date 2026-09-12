import { ArrowPathIcon } from "@heroicons/react/24/outline";
import { Button, Card } from "@heroui/react";
import { SettingsInput, settingsInputClass, settingsLabelClass } from "../Settings/fields";
import type { UseSettingsConfigReturn } from "../Settings/useSettingsConfig";

interface ClashApiCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * Clash API（ADR-0005 必选切片，2026-09 起并入 Experimental 页）。
 *
 * 功能恒启用（`clash_api_enabled` 由页面层强制为 true），不提供关闭开关：
 * - `clash_api_port` 数字输入（仅 1-65535 合法才落库，非法规格展示行内错误）；
 * - `clash_api_secret` **必填**密钥（面板跳转携带密钥，不允许为空）：空值只提示不落库，
 *   右侧按钮随机生成并立即落库；
 * - 两字段均即时保存（防抖/立即），核心重启后生效。
 */
export function ClashApiCard({ settings }: ClashApiCardProps) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>Clash API</Card.Title>
        <Card.Description>仪表盘与面板页的数据源；端口与密钥修改即时保存，重启代理后生效</Card.Description>
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
        <div className="flex flex-col gap-2">
          <label htmlFor="settings-clash-api-secret" className={settingsLabelClass}>
            密钥（必填）
          </label>
          <div className="flex items-center gap-2">
            <input
              id="settings-clash-api-secret"
              aria-label="密钥"
              type="password"
              autoComplete="off"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
              value={settings.clashApiSecretDraft}
              placeholder="填写或随机生成密钥"
              disabled={!settings.ready}
              onChange={(event) => settings.onClashApiSecretChange(event.target.value)}
              className={`${settingsInputClass} min-w-0 flex-1`}
            />
            <Button
              variant="secondary"
              className="h-12 shrink-0 gap-1.5 px-3"
              isDisabled={!settings.ready}
              onPress={settings.onGenerateClashApiSecret}
            >
              <ArrowPathIcon className="size-4" aria-hidden="true" />
              随机生成
            </Button>
          </div>
          {settings.clashApiSecretError ? (
            <span className="text-xs text-warning">{settings.clashApiSecretError}</span>
          ) : (
            <span className="text-xs text-muted">面板访问需携带该密钥鉴权</span>
          )}
        </div>
      </Card.Content>
    </Card>
  );
}
