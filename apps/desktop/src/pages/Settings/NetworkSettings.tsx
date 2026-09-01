import { Alert, Button, Card, Chip, Input, Label, ListBox, Select, Switch } from "@heroui/react";
import type { UseSettingsConfigReturn } from "./useSettingsConfig";
import { TUN_STACK_OPTIONS } from "./useSettingsConfig";
import { authorizeTun } from "../../api";
import { toErrorMessage } from "../../api";

interface NetworkSettingsProps {
  settings: UseSettingsConfigReturn;
}

export default function NetworkSettings({ settings }: NetworkSettingsProps) {
  const {
    isAndroid,
    mixedPort,
    setMixedPort,
    tunEnabled,
    setTunEnabled,
    tunStack,
    setTunStack,
    tunAutoRoute,
    setTunAutoRoute,
    tunAuth,
    tunAuthError,
    tunAuthBusy,
    setTunAuthBusy,
    setTunAuth,
    setTunAuthError,
    persist,
    persistDebounced,
  } = settings;

  const tunAuthReason = tunAuth?.startsWith("unsupported:") ? tunAuth.slice("unsupported:".length) : null;

  const handleAuthorizeTun = async () => {
    setTunAuthBusy(true);
    setTunAuthError(null);
    try {
      setTunAuth(await authorizeTun());
    } catch (err) {
      setTunAuthError(toErrorMessage(err));
    } finally {
      setTunAuthBusy(false);
    }
  };

  return (
    <Card>
      <Card.Header>
        <Card.Title>网络设置</Card.Title>
        <Card.Description>
          {isAndroid ? "本地混合端口与 VPN 服务（TUN）配置" : "本地混合端口与虚拟网卡（TUN）配置"}
        </Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="flex flex-col gap-2">
          <Label htmlFor="settings-mixed-port">混合端口</Label>
          <Input
            id="settings-mixed-port"
            aria-label="混合端口"
            type="number"
            min={1}
            max={65535}
            value={String(mixedPort)}
            onChange={(event) => {
              const parsed = Number(event.target.value);
              const next = Number.isFinite(parsed) ? parsed : 0;
              setMixedPort(next);
              persistDebounced({ mixed_port: next });
            }}
            fullWidth
          />
        </div>

        {/* TUN 区：桌面以开关启用并需提权授权；Android 由 VpnService（TUN）恒接管
            全部流量，以静态说明替代开关且不展示提权相关区域。协议栈与自动路由两平台
            均可编辑（持久化后参与合成 tun 入站）。 */}
        {isAndroid ? (
          <span className="text-sm text-muted">Android 始终通过 VPN 服务（TUN）接管全部流量</span>
        ) : (
          <Switch
            isSelected={tunEnabled}
            onChange={(next) => {
              setTunEnabled(next);
              void persist({ tun_enabled: next });
            }}
          >
            <Switch.Content>
              <Switch.Control>
                <Switch.Thumb />
              </Switch.Control>
              启用 TUN 模式
            </Switch.Content>
          </Switch>
        )}

        {/* TUN 授权区仅桌面展示：Android 由 VpnService 系统授权，无提权语义。 */}
        {!isAndroid && tunEnabled && (
          <div className="flex flex-col gap-3">
            {tunAuth === "authorized" && (
              <div className="flex items-center gap-2">
                <Chip size="sm" variant="soft" color="success">
                  已授权
                </Chip>
                <span className="text-xs text-muted">核心已具备 TUN 提权能力</span>
              </div>
            )}

            {tunAuth === "needs_auth" && (
              <Alert status="warning">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>TUN 需要系统授权</Alert.Title>
                  <Alert.Description>
                    当前核心未获得 TUN 提权，授权后才能接管全部流量（失败时按错误提示处理：Linux 安装 polkit、Windows
                    以管理员身份重启应用）。
                  </Alert.Description>
                  <div className="mt-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      isPending={tunAuthBusy}
                      onPress={() => void handleAuthorizeTun()}
                    >
                      立即授权
                    </Button>
                  </div>
                </Alert.Content>
              </Alert>
            )}

            {tunAuthReason && (
              <Alert status="default">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>TUN 授权不可用</Alert.Title>
                  <Alert.Description>{tunAuthReason}</Alert.Description>
                </Alert.Content>
              </Alert>
            )}

            {tunAuthError && (
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>授权失败</Alert.Title>
                  <Alert.Description>{tunAuthError}</Alert.Description>
                </Alert.Content>
              </Alert>
            )}
          </div>
        )}

        <div className="grid gap-4 sm:grid-cols-2">
          <div className="flex flex-col gap-2">
            <Label htmlFor="settings-tun-stack">协议栈</Label>
            <Select
              id="settings-tun-stack"
              aria-label="协议栈"
              value={tunStack}
              onChange={(value) => {
                const next = String(value ?? "mixed");
                setTunStack(next);
                void persist({ tun_stack: next });
              }}
              fullWidth
            >
              <Select.Trigger>
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  {TUN_STACK_OPTIONS.map((option) => (
                    <ListBox.Item key={option.id} id={option.id} textValue={option.label}>
                      {option.label}
                      <ListBox.ItemIndicator />
                    </ListBox.Item>
                  ))}
                </ListBox>
              </Select.Popover>
            </Select>
          </div>
          <div className="flex items-end">
            <Switch
              isSelected={tunAutoRoute}
              onChange={(next) => {
                setTunAutoRoute(next);
                void persist({ tun_auto_route: next });
              }}
            >
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
                自动路由
              </Switch.Content>
            </Switch>
          </div>
        </div>

        {/* 权限说明仅桌面展示：Android 无 TUN 提权概念。 */}
        {!isAndroid && (
          <Alert status="default">
            <Alert.Indicator />
            <Alert.Content>
              <Alert.Title>权限说明</Alert.Title>
              <Alert.Description>
                TUN 模式需要管理员 / root 权限；设置页的 TUN / Clash 面板配置优先级高于协议配置中的覆写
              </Alert.Description>
            </Alert.Content>
          </Alert>
        )}
      </Card.Content>
    </Card>
  );
}
