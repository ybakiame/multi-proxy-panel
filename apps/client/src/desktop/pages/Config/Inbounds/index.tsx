import { Alert, Button, Card, Chip, Input, Label, ListBox, Select, Switch } from "@heroui/react";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { authorizeTun, toErrorMessage } from "@pp/client-core";
import { TUN_STACK_OPTIONS, useSettingsConfig } from "../../Settings/useSettingsConfig";

/**
 * 入站管理页（桌面端，路由 `/config/inbounds`；语义对齐移动端
 * `apps/mobile/src/pages/Config/InboundsPage.tsx`，对应 sing-box 顶级 `inbounds` 字段）。
 *
 * 数据流：与移动端一致——入站参数不落配置切片，而是作为高优先级设置
 * （`client.json` 的 `mixed_port` / `tun_*` / `ipv6_enabled`）在配置合成 ④ 层
 * 强制覆盖模板/覆写中的同名字段（`apply_singbox_panel_features`）；字段经
 * `useSettingsConfig` 即时保存，重启代理后生效。
 *
 * 结构按核心最终配置的两条入站组织：
 * - 混合入站（`mixed-in`）：`listen` 固定 127.0.0.1，仅可调 `listen_port`（mixed_port）；
 * - TUN 入站（`tun-in`）：桌面端为**可选**模式（开关 + 系统提权授权），`stack` /
 *   `auto_route` 可调（双栈地址与 MTU 9000 为内置不可调）；IPv6 开关控制双栈承载
 *   与 DNS AAAA。
 */
export default function InboundsPage() {
  const navigate = useNavigate();
  const settings = useSettingsConfig();
  const {
    mixedPortDraft,
    mixedPortError,
    onMixedPortChange,
    ipv6Enabled,
    setIpv6Enabled,
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
  } = settings;

  const tunAuthReason = tunAuth?.startsWith("unsupported:") ? tunAuth.slice("unsupported:".length) : null;

  const handleAuthorizeTun = async () => {
    setTunAuthBusy(true);
    setTunAuthError(null);
    try {
      setTunAuth(await authorizeTun());
    } catch (err) {
      setTunAuthError(toErrorMessage(err));
    }
    setTunAuthBusy(false);
  };

  return (
    <div className="flex max-w-xl flex-col gap-6">
      {/* 页头：返回 + 标题 */}
      <div className="flex items-center gap-2">
        <Button size="sm" variant="ghost" isIconOnly aria-label="返回配置管理" onPress={() => navigate("/config")}>
          <ArrowLeftIcon className="size-4" />
        </Button>
        <div>
          <h1 className="text-xl font-semibold">入站管理</h1>
          <p className="text-sm text-muted">Inbounds 中的端口与 TUN 参数；修改即时保存，重启代理后生效</p>
        </div>
      </div>

      <Alert status="default">
        <Alert.Indicator />
        <Alert.Content>
          <Alert.Description>
            仅支持部分参数调整；入站设置为高优先级覆盖，优先于订阅模板与覆写中的同名字段
          </Alert.Description>
        </Alert.Content>
      </Alert>

      {/* 混合入站（mixed-in）：listen 固定 127.0.0.1，仅 listen_port 可调 */}
      <Card>
        <Card.Header>
          <Card.Title>混合入站</Card.Title>
          <Card.Description>HTTP / SOCKS 混合代理入站，监听 127.0.0.1</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <div className="flex flex-col gap-2">
            <Label htmlFor="inbounds-mixed-port">监听端口</Label>
            <Input
              id="inbounds-mixed-port"
              aria-label="混合端口"
              type="number"
              min={1}
              max={65535}
              value={mixedPortDraft}
              onChange={(event) => onMixedPortChange(event.target.value)}
              placeholder="17890"
              fullWidth
            />
            {mixedPortError ? (
              <span className="text-xs text-danger">{mixedPortError}</span>
            ) : (
              <span className="text-xs text-muted">其他应用手动配置代理（HTTP / SOCKS）时使用的本地端口</span>
            )}
          </div>
        </Card.Content>
      </Card>

      {/* TUN 入站（tun-in）：桌面端可选模式；stack / auto_route 可调，双栈地址与 MTU 内置固定 */}
      <Card>
        <Card.Header>
          <Card.Title>TUN 入站（tun-in）</Card.Title>
          <Card.Description>虚拟网卡接管系统流量；桌面端为可选模式，需要系统提权授权</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
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

          {/* TUN 授权区：仅启用时展示（未启用 TUN 不需要任何提权）。 */}
          {tunEnabled && (
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
              <Label htmlFor="inbounds-tun-stack">协议栈</Label>
              <Select
                id="inbounds-tun-stack"
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

          {/* IPv6 开关：控制 TUN 双栈承载与 DNS AAAA（语义对齐移动端入站管理页）。 */}
          <div className="flex flex-col gap-1">
            <Switch
              isSelected={ipv6Enabled}
              onChange={(next) => {
                setIpv6Enabled(next);
                void persist({ ipv6_enabled: next });
              }}
            >
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
                IPv6 支持
              </Switch.Content>
            </Switch>
            <span className="text-xs text-muted">
              关闭时 DNS 不返回 AAAA 记录（推荐，节点不支持 IPv6 时最稳定）；开启后双栈，IPv6 流量经隧道按规则分流
            </span>
          </div>

          {/* 权限说明 */}
          <Alert status="default">
            <Alert.Indicator />
            <Alert.Content>
              <Alert.Title>权限说明</Alert.Title>
              <Alert.Description>
                TUN 模式需要管理员 / root 权限；仅在「启用 TUN
                模式」开启后启动代理时才需要授权，未启用不影响其他代理方式
              </Alert.Description>
            </Alert.Content>
          </Alert>
        </Card.Content>
      </Card>
    </div>
  );
}
