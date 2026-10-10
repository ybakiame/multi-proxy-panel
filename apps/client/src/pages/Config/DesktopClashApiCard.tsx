import { Alert, Card, Input, Label, ListBox, Select, Switch } from "@pp/ui";
import { CLASH_UI_OPTIONS } from "../Settings/desktop/useSettingsConfig";
import { useSettingsConfig } from "../Settings/desktop/useSettingsConfig";
import type { UseSettingsConfigReturn } from "../Settings/desktop/useSettingsConfig";

interface ClashApiCardProps {
  settings: UseSettingsConfigReturn;
}

/**
 * Clash 面板卡（桌面端，2026-10 自「设置」页迁入「配置 → 实验性配置」页，
 * 语义对齐移动端 `Config/ClashApiCard.tsx`）。
 *
 * Clash API 不是配置切片：作为高优先级设置（`client.json`）在配置合成 ④ 层
 * 整体覆盖模板/覆写中的 `experimental.clash_api` 同名字段；桌面端保留启用开关
 * 与可选密钥（区别于移动端的恒启用 + 必填密钥）。字段即时保存
 * （useSettingsConfig），重启代理后生效，不经本页切片草稿的保存按钮。
 */
export default function ClashApiCard({ settings }: ClashApiCardProps) {
  const {
    clashApiEnabled,
    setClashApiEnabled,
    clashApiPortDraft,
    clashApiPortError,
    onClashApiPortChange,
    clashApiSecret,
    setClashApiSecret,
    clashApiUi,
    setClashApiUi,
    persist,
    persistDebounced,
  } = settings;

  return (
    <Card>
      <Card.Header>
        <Card.Title>Clash API</Card.Title>
        <Card.Description>通过本地面板 API 查看连接与切换节点</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <Switch
          isSelected={clashApiEnabled}
          onChange={(next) => {
            setClashApiEnabled(next);
            void persist({ clash_api_enabled: next });
          }}
        >
          <Switch.Content>
            <Switch.Control>
              <Switch.Thumb />
            </Switch.Control>
            启用 Clash 面板 API
          </Switch.Content>
        </Switch>

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="flex flex-col gap-2">
            <Label htmlFor="experimental-clash-port">端口</Label>
            <Input
              id="experimental-clash-port"
              aria-label="端口"
              type="number"
              min={1}
              max={65535}
              value={clashApiPortDraft}
              onChange={(event) => onClashApiPortChange(event.target.value)}
              placeholder="9090"
              fullWidth
            />
            {clashApiPortError && <span className="text-xs text-danger">{clashApiPortError}</span>}
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="experimental-clash-secret">密钥（可选）</Label>
            <Input
              id="experimental-clash-secret"
              aria-label="密钥（可选）"
              type="password"
              value={clashApiSecret}
              onChange={(event) => {
                setClashApiSecret(event.target.value);
                persistDebounced({ clash_api_secret: event.target.value });
              }}
              placeholder="留空则不鉴权"
              fullWidth
            />
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <Label htmlFor="experimental-clash-ui">面板 UI</Label>
          <Select
            id="experimental-clash-ui"
            aria-label="面板 UI"
            value={clashApiUi}
            onChange={(value) => {
              const next = String(value ?? "zashboard");
              setClashApiUi(next);
              void persist({ clash_api_ui: next });
            }}
            fullWidth
          >
            <Select.Trigger>
              <Select.Value />
              <Select.Indicator />
            </Select.Trigger>
            <Select.Popover>
              <ListBox>
                {CLASH_UI_OPTIONS.map((option) => (
                  <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                    {option.label}
                  </ListBox.Item>
                ))}
              </ListBox>
            </Select.Popover>
          </Select>
          <span className="text-xs text-muted">首次访问面板地址时自动下载所选面板资源</span>
        </div>

        <Alert status="default">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>访问方式</Alert.Title>
            <Alert.Description>
              面板地址 http://127.0.0.1:{clashApiPortDraft}/ui，默认 {clashApiUi}，可切换 yacd / metacubexd
            </Alert.Description>
          </Alert.Content>
        </Alert>
      </Card.Content>
    </Card>
  );
}

export function DesktopClashApiSettings() {
  return <ClashApiCard settings={useSettingsConfig()} />;
}
