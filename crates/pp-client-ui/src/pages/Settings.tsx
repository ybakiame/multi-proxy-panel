import { useEffect, useState } from "react";
import { Alert, Button, Card, Input, Label, ListBox, Select, Switch } from "@heroui/react";
import type { ClientConfig } from "../api";
import { useAppStore } from "../store";

const CORE_TYPE_OPTIONS = [
  { id: "singbox", label: "SingBox" },
  { id: "mihomo", label: "Mihomo" },
] as const;

/** 兼容任务描述中的 PascalCase 值（`SingBox`/`Mihomo`）与后端 serde 值（`singbox`/`mihomo`）。 */
function normalizeCoreType(value: string): string {
  if (value === "SingBox") {
    return "singbox";
  }
  if (value === "Mihomo") {
    return "mihomo";
  }
  return value;
}

export default function Settings() {
  const { config, loading, error, clearError, loadConfig, saveConfig } = useAppStore();
  const [hubUrl, setHubUrl] = useState("");
  const [subToken, setSubToken] = useState("");
  const [coreType, setCoreType] = useState<string>("singbox");
  const [coreBinary, setCoreBinary] = useState("");
  const [mixedPort, setMixedPort] = useState(1080);
  const [mitmEnabled, setMitmEnabled] = useState(false);
  const [systemProxyEnabled, setSystemProxyEnabled] = useState(false);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    if (!config) {
      return;
    }
    setHubUrl(config.hub_url);
    setSubToken(config.sub_token);
    setCoreType(normalizeCoreType(config.core_type));
    setCoreBinary(config.core_binary);
    setMixedPort(config.mixed_port);
    setMitmEnabled(config.mitm_enabled);
    setSystemProxyEnabled(config.system_proxy_enabled);
  }, [config]);

  const handleSave = async () => {
    if (!config) {
      return;
    }
    setSaved(false);
    clearError();
    const payload: ClientConfig = {
      ...config,
      hub_url: hubUrl.trim(),
      sub_token: subToken.trim(),
      core_type: coreType,
      core_binary: coreBinary,
      mixed_port: mixedPort,
      mitm_enabled: mitmEnabled,
      system_proxy_enabled: systemProxyEnabled,
    };
    try {
      await saveConfig(payload);
      setSaved(true);
    } catch {
      // 错误已由 store 记录并展示。
    }
  };

  const canSave = hubUrl.trim().length > 0 && subToken.trim().length > 0;

  return (
    <div className="flex max-w-xl flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端连接与核心运行配置</p>
      </div>

      <Card>
        <Card.Header>
          <Card.Title>基本配置</Card.Title>
          <Card.Description>保存后写入数据目录的 client.json</Card.Description>
        </Card.Header>
        <Card.Content>
          <div className="flex flex-col gap-4">
            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-hub-url">Hub 地址</Label>
              <Input
                id="settings-hub-url"
                value={hubUrl}
                onChange={(event) => setHubUrl(event.target.value)}
                placeholder="https://hub.example.com"
                fullWidth
              />
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-sub-token">订阅 Token</Label>
              <Input
                id="settings-sub-token"
                type="password"
                value={subToken}
                onChange={(event) => setSubToken(event.target.value)}
                placeholder="sub token"
                fullWidth
              />
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <div className="flex flex-col gap-2">
                <Label>核心类型</Label>
                <Select value={coreType} onChange={(key) => setCoreType(String(key))} fullWidth>
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      {CORE_TYPE_OPTIONS.map((option) => (
                        <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                          {option.label}
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))}
                    </ListBox>
                  </Select.Popover>
                </Select>
              </div>

              <div className="flex flex-col gap-2">
                <Label htmlFor="settings-core-binary">核心二进制路径</Label>
                <Input
                  id="settings-core-binary"
                  value={coreBinary}
                  onChange={(event) => setCoreBinary(event.target.value)}
                  placeholder="留空则自动定位"
                  fullWidth
                />
              </div>
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="settings-mixed-port">混合端口</Label>
              <Input
                id="settings-mixed-port"
                type="number"
                min={1}
                max={65535}
                value={String(mixedPort)}
                onChange={(event) => {
                  const parsed = Number(event.target.value);
                  setMixedPort(Number.isFinite(parsed) ? parsed : 0);
                }}
                fullWidth
              />
            </div>

            <div className="flex flex-col gap-3 rounded-xl border border-border/60 bg-surface p-4">
              <Switch isSelected={mitmEnabled} onChange={setMitmEnabled}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  启用 MITM
                </Switch.Content>
              </Switch>
              <Switch isSelected={systemProxyEnabled} onChange={setSystemProxyEnabled}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  启用系统代理
                </Switch.Content>
              </Switch>
            </div>
          </div>
        </Card.Content>
        <Card.Footer>
          <Button variant="primary" isPending={loading} isDisabled={!canSave} onPress={() => void handleSave()}>
            保存配置
          </Button>
          {saved && <span className="text-sm text-success">已保存</span>}
        </Card.Footer>
      </Card>

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>保存失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
