import { Alert, Card, Input, Label, ListBox, Select, Switch } from "@heroui/react";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { CLASH_UI_OPTIONS } from "./useSettingsConfig";

interface ClashPanelSettingsProps {
  settings: UseSettingsConfigReturn;
}

export default function ClashPanelSettings({ settings }: ClashPanelSettingsProps) {
  const {
    clashApiEnabled,
    setClashApiEnabled,
    clashApiPort,
    setClashApiPort,
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
        <Card.Title>Clash 面板</Card.Title>
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
            <Label htmlFor="settings-clash-port">端口</Label>
            <Input
              id="settings-clash-port"
              aria-label="端口"
              type="number"
              min={1}
              max={65535}
              value={String(clashApiPort)}
              onChange={(event) => {
                const parsed = Number(event.target.value);
                const next = Number.isFinite(parsed) ? parsed : 0;
                setClashApiPort(next);
                persistDebounced({ clash_api_port: next });
              }}
              fullWidth
            />
          </div>
          <div className="flex flex-col gap-2">
            <Label htmlFor="settings-clash-secret">密钥（可选）</Label>
            <Input
              id="settings-clash-secret"
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
          <Label htmlFor="settings-clash-ui">面板 UI</Label>
          <Select
            id="settings-clash-ui"
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
                    <ListBox.ItemIndicator />
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
              面板地址 http://127.0.0.1:{clashApiPort}/ui，默认 {clashApiUi}，可切换 yacd / metacubexd
            </Alert.Description>
          </Alert.Content>
        </Alert>
      </Card.Content>
    </Card>
  );
}
