import { Button, Card, Label, ListBox, Select, Switch } from "@pp/ui";
import type { ClientConfig, LocalCoreView, SubscriptionView } from "@pp/client-core";
interface Props {
  config: ClientConfig | null | undefined;
  enabledSubs: SubscriptionView[];
  cores: LocalCoreView[];
  activeCore: LocalCoreView | null;
  gateMessages: string[];
  legacyHub: boolean;
  canPreview: boolean;
  canStart: boolean;
  running: boolean;
  busy: "start" | "stop" | null;
  loading: boolean;
  onSelectSubscription: (id: string) => Promise<void>;
  onSelectCore: (path: string) => Promise<void>;
  onPersist: (patch: Partial<ClientConfig>) => Promise<void>;
  onPreview: () => void;
  onStart: () => Promise<void>;
  onStop: () => Promise<void>;
}
/** 桌面运行配置与启动门禁的展示区，查询和操作由仪表盘模型负责。 */
export function DesktopRunConfig({
  config,
  enabledSubs,
  cores,
  activeCore,
  gateMessages,
  legacyHub,
  canPreview,
  canStart,
  running,
  busy,
  loading,
  onSelectSubscription,
  onSelectCore,
  onPersist,
  onPreview,
  onStart,
  onStop,
}: Props) {
  return (
    <Card>
      <Card.Header>
        <Card.Title>运行配置</Card.Title>
        <Card.Description>选择生效订阅与核心二进制，满足门禁后可启动代理</Card.Description>
      </Card.Header>
      <Card.Content className="flex flex-col gap-4">
        <div className="grid gap-4 sm:grid-cols-2">
          <div className="flex flex-col gap-2">
            <Label htmlFor="dashboard-subscription">生效订阅</Label>
            {enabledSubs.length === 0 ? (
              <span className="text-xs text-muted">先到「订阅」页添加并启用订阅</span>
            ) : (
              <Select
                id="dashboard-subscription"
                aria-label="生效订阅"
                value={config?.active_subscription_id ?? ""}
                onChange={(key) => void onSelectSubscription(String(key ?? ""))}
                placeholder="请选择订阅"
                fullWidth
              >
                <Select.Trigger>
                  <Select.Value />
                  <Select.Indicator />
                </Select.Trigger>
                <Select.Popover>
                  <ListBox>
                    {enabledSubs.map((sub) => (
                      <ListBox.Item key={sub.id} id={sub.id} textValue={sub.name}>
                        {sub.name} · {sub.node_count} 节点
                      </ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            )}
          </div>

          <div className="flex flex-col gap-2">
            <Label htmlFor="dashboard-core">核心</Label>
            {/* 桌面：单核心（sing-box），列出全部可用二进制。 */}
            <Select
              key="core-desktop"
              id="dashboard-core"
              aria-label="核心二进制"
              value={activeCore?.path ?? ""}
              onChange={(key) => void onSelectCore(String(key ?? ""))}
              placeholder="请选择核心"
              isDisabled={cores.length === 0}
              fullWidth
            >
              <Select.Trigger>
                <Select.Value />
                <Select.Indicator />
              </Select.Trigger>
              <Select.Popover>
                <ListBox>
                  {cores.length === 0 ? (
                    <ListBox.Item key="__empty" id="__empty" textValue="暂无可用核心">
                      暂无可用核心
                    </ListBox.Item>
                  ) : (
                    cores.map((core) => (
                      <ListBox.Item key={core.path} id={core.path} textValue={`sing-box ${core.version}`}>
                        sing-box {core.version}
                      </ListBox.Item>
                    ))
                  )}
                </ListBox>
              </Select.Popover>
            </Select>
          </div>
        </div>

        {gateMessages.map((message) => (
          <span key={message} className="text-xs text-warning">
            {message}
          </span>
        ))}
        {legacyHub && (
          <span className="text-xs text-warning">使用旧版 Hub 订阅（deprecated），建议到「订阅」页添加订阅</span>
        )}

        {/* 系统代理 / MITM 开关（桌面由核心直接接管系统流量）。 */}
        <div className="flex flex-col gap-3">
          <div className="flex flex-col gap-1">
            <Switch
              isSelected={config?.mitm_enabled ?? false}
              isDisabled={!config || loading || busy !== null}
              onChange={(next) => void onPersist({ mitm_enabled: next })}
            >
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
                启用 MITM
              </Switch.Content>
            </Switch>
            <span className="text-xs text-muted">拦截并解密 HTTPS 流量（重写/脚本钩子），重启代理生效</span>
          </div>
          <div className="flex flex-col gap-1">
            <Switch
              isSelected={config?.system_proxy_enabled ?? false}
              isDisabled={!config || loading || busy !== null}
              onChange={(next) => void onPersist({ system_proxy_enabled: next })}
            >
              <Switch.Content>
                <Switch.Control>
                  <Switch.Thumb />
                </Switch.Control>
                启用系统代理
              </Switch.Content>
            </Switch>
            <span className="text-xs text-muted">接管系统代理设置指向核心 mixed 入口，随代理启停生效</span>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-4">
          <Button variant="secondary" size="lg" isDisabled={!canPreview} onPress={() => onPreview()}>
            配置预览
          </Button>
          {running ? (
            <Button
              variant="danger"
              size="lg"
              isPending={loading || busy === "stop"}
              isDisabled={busy === "start"}
              onPress={() => void onStop()}
            >
              停止代理
            </Button>
          ) : (
            <Button
              variant="primary"
              size="lg"
              isPending={loading || busy === "start"}
              isDisabled={!canStart || busy === "stop"}
              onPress={() => void onStart()}
            >
              启动代理
            </Button>
          )}
          <span className="text-sm text-muted">启动后由后端执行订阅同步并拉起核心，配置可在「设置」页修改。</span>
        </div>
      </Card.Content>
    </Card>
  );
}
