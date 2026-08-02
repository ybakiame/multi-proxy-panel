import { useCallback, useEffect, useState } from "react";
import { Alert, Button, Card, Chip, Label, ListBox, Select, Switch } from "@heroui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  listCores,
  listProfiles,
  listSubscriptions,
  setActiveCore,
  setActiveSubscription,
  setRuleMode as setRuleModeApi,
  toErrorMessage,
} from "../api";
import type { ClientConfig, CoreType, LocalCoreView, ProfileView, SubscriptionView } from "../api";
import { useAppStore } from "../store";

const CORE_LABELS: Record<CoreType, string> = {
  singbox: "sing-box",
  mihomo: "mihomo",
};

/** 规则模式按钮（与后端 `rule` / `global` / `direct` 对齐）。 */
const RULE_MODES = [
  { id: "rule", label: "规则" },
  { id: "global", label: "全局" },
  { id: "direct", label: "直连" },
] as const;

/** 核心类型展示名（兼容 `singbox` / `mihomo` 小写 serde 值）。 */
function coreLabel(value: string): string {
  return CORE_LABELS[(value === "SingBox" ? "singbox" : value === "Mihomo" ? "mihomo" : value) as CoreType] ?? value;
}

export default function Dashboard() {
  const { config, status, loading, error, loadConfig, refreshStatus, saveConfig, start, stop, setStatus } =
    useAppStore();
  const [busy, setBusy] = useState<"start" | "stop" | null>(null);
  const [subs, setSubs] = useState<SubscriptionView[]>([]);
  const [cores, setCores] = useState<LocalCoreView[]>([]);
  const [profiles, setProfiles] = useState<ProfileView[]>([]);
  const [actionError, setActionError] = useState<string | null>(null);
  const [ruleModeBusy, setRuleModeBusy] = useState<string | null>(null);
  const [linkCopied, setLinkCopied] = useState(false);
  const [saveWarning, setSaveWarning] = useState<string | null>(null);

  /**
   * 运行配置开关的即时保存：从 store 取最新配置叠加补丁（避免闭包旧值），
   * 成功返回后端 warning，失败时 store.error 由页面级 Alert 展示并 `loadConfig()` 回滚。
   */
  const persistConfig = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const current = useAppStore.getState().config;
      if (!current) {
        return;
      }
      setSaveWarning(null);
      try {
        const warning = await saveConfig({ ...current, ...patch });
        setSaveWarning(warning);
      } catch {
        await loadConfig();
      }
    },
    [saveConfig, loadConfig],
  );

  const loadSubscriptions = useCallback(async () => {
    try {
      setSubs(await listSubscriptions());
    } catch (err) {
      setActionError(toErrorMessage(err));
    }
  }, []);

  const loadCores = useCallback(async () => {
    try {
      setCores(await listCores());
    } catch (err) {
      setActionError(toErrorMessage(err));
    }
  }, []);

  const loadProfiles = useCallback(async () => {
    try {
      setProfiles(await listProfiles());
    } catch (err) {
      setActionError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void loadConfig();
    void refreshStatus();
    void loadSubscriptions();
    void loadCores();
    void loadProfiles();
    // 状态轮询：每 2s 刷新一次运行状态。
    const timer = window.setInterval(() => {
      void refreshStatus();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [loadConfig, refreshStatus, loadSubscriptions, loadCores, loadProfiles]);

  const handleStart = async () => {
    setBusy("start");
    try {
      await start();
    } finally {
      setBusy(null);
    }
  };

  const handleStop = async () => {
    setBusy("stop");
    try {
      await stop();
    } finally {
      setBusy(null);
    }
  };

  const handleSelectSubscription = async (id: string) => {
    try {
      await setActiveSubscription(id);
      setActionError(null);
      await loadConfig();
    } catch (err) {
      setActionError(toErrorMessage(err));
    }
  };

  const handleSelectCore = async (path: string) => {
    try {
      await setActiveCore(path);
      setActionError(null);
      await loadConfig();
      await loadCores();
    } catch (err) {
      setActionError(toErrorMessage(err));
    }
  };

  const handleRuleMode = async (mode: string) => {
    setRuleModeBusy(mode);
    try {
      const next = await setRuleModeApi(mode);
      setStatus(next);
      setActionError(null);
    } catch (err) {
      setActionError(toErrorMessage(err));
    } finally {
      setRuleModeBusy(null);
    }
  };

  const handleCopyLink = async () => {
    if (!config) {
      return;
    }
    const secret = config.clash_api_secret || "";
    const url = `http://127.0.0.1:${config.clash_api_port}/ui/?hostname=127.0.0.1&port=${config.clash_api_port}${secret ? `&secret=${secret}` : ""}`;
    await navigator.clipboard.writeText(url);
    setLinkCopied(true);
    window.setTimeout(() => setLinkCopied(false), 2000);
  };

  const handleOpenPanel = async () => {
    if (!config) {
      return;
    }
    try {
      await openUrl(`http://127.0.0.1:${config.clash_api_port}/ui`);
    } catch (err) {
      setActionError(toErrorMessage(err));
    }
  };

  const running = status?.core_running ?? false;
  // start_proxy 在 TUN 未授权时返回 `tun_auth_required` 错误，改为引导前往设置页授权。
  const tunAuthRequired = error?.includes("tun_auth_required") ?? false;
  const alertError = error ?? actionError;

  // 运行门禁：不满足时禁止启动并逐条提示。
  const enabledSubs = subs.filter((sub) => sub.enabled);
  const activeSub = subs.find((sub) => sub.id === config?.active_subscription_id) ?? null;
  const activeCore = cores.find((core) => core.active) ?? null;

  const gateMessages: string[] = [];
  if (!activeSub) {
    gateMessages.push("请先选择要使用的订阅");
  }
  if (!config?.core_binary || !activeCore) {
    gateMessages.push("请先选择要使用的核心");
  }
  if (activeSub && activeSub.format === "ClashYaml" && config?.core_type === "singbox") {
    gateMessages.push("该订阅为 Clash 格式，需切换 mihomo 核心");
  }
  if (activeSub?.profile_id) {
    const profile = profiles.find((p) => p.id === activeSub.profile_id);
    if (profile && config && profile.core_type !== config.core_type) {
      gateMessages.push(`关联覆写适用于 ${coreLabel(profile.core_type)}，与当前核心不匹配`);
    }
  }
  const canStart = gateMessages.length === 0;

  // 规则模式：优先取运行状态，其次配置，默认 rule。
  const ruleMode = status?.rule_mode ?? config?.rule_mode ?? "rule";
  const ruleModeHint =
    running && config?.clash_api_enabled
      ? "即时生效"
      : config?.core_type === "singbox"
        ? "已保存，将在下次启动生效（sing-box 运行时切换依赖 Clash 面板 API，需在「设置 → Clash 面板」开启）"
        : "已保存，将在下次启动生效";

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">仪表盘</h1>
        <p className="text-sm text-muted">代理核心运行状态与启停控制</p>
      </div>

      {alertError && !tunAuthRequired && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{alertError}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {tunAuthRequired && (
        <Alert status="warning">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>需要 TUN 授权</Alert.Title>
            <Alert.Description>
              代理启动失败：TUN 模式未获得系统授权。请前往「设置 → TUN 模式」点击「立即授权」后重新启动代理。
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {/* A. 运行配置 */}
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
                  value={config?.active_subscription_id ?? ""}
                  onChange={(key) => void handleSelectSubscription(String(key ?? ""))}
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
                          <ListBox.ItemIndicator />
                        </ListBox.Item>
                      ))}
                    </ListBox>
                  </Select.Popover>
                </Select>
              )}
            </div>

            <div className="flex flex-col gap-2">
              <Label htmlFor="dashboard-core">核心</Label>
              <Select
                id="dashboard-core"
                value={activeCore?.path ?? ""}
                onChange={(key) => void handleSelectCore(String(key ?? ""))}
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
                      <ListBox.Item id="__empty" textValue="暂无可用核心">
                        暂无可用核心
                      </ListBox.Item>
                    ) : (
                      cores.map((core) => (
                        <ListBox.Item key={core.path} id={core.path} textValue={coreLabel(core.core_type)}>
                          {coreLabel(core.core_type)} {core.version}
                          <ListBox.ItemIndicator />
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

          <div className="flex flex-col gap-3">
            <div className="flex flex-col gap-1">
              <Switch
                isSelected={config?.mitm_enabled ?? false}
                isDisabled={!config || loading || busy !== null}
                onChange={(next) => void persistConfig({ mitm_enabled: next })}
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
                onChange={(next) => void persistConfig({ system_proxy_enabled: next })}
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
            {saveWarning && <span className="text-xs text-warning">{saveWarning}</span>}
          </div>

          <div className="flex items-center gap-4">
            {running ? (
              <Button
                variant="danger"
                size="lg"
                isPending={loading || busy === "stop"}
                isDisabled={busy === "start"}
                onPress={() => void handleStop()}
              >
                停止代理
              </Button>
            ) : (
              <Button
                variant="primary"
                size="lg"
                isPending={loading || busy === "start"}
                isDisabled={!canStart || busy === "stop"}
                onPress={() => void handleStart()}
              >
                启动代理
              </Button>
            )}
            <span className="text-sm text-muted">启动后由后端执行订阅同步并拉起核心，配置可在「设置」页修改。</span>
          </div>
        </Card.Content>
      </Card>

      {/* B. 规则模式 */}
      <Card>
        <Card.Header>
          <Card.Title>规则模式</Card.Title>
          <Card.Description>切换流量路由策略</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-3">
          <div className="flex items-center gap-2">
            {RULE_MODES.map((mode) => (
              <Button
                key={mode.id}
                size="sm"
                variant={ruleMode === mode.id ? "primary" : "secondary"}
                isPending={ruleModeBusy === mode.id}
                isDisabled={ruleModeBusy !== null}
                onPress={() => void handleRuleMode(mode.id)}
              >
                {mode.label}
              </Button>
            ))}
          </div>
          <span className="text-xs text-muted">{ruleModeHint}</span>
        </Card.Content>
      </Card>

      {/* C. 状态卡片 */}
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        <Card>
          <Card.Header>
            <Card.Title>核心状态</Card.Title>
            <Card.Description>
              {config?.core_type === "mihomo" ? "mihomo" : "sing-box"}
              {config ? ` · 混合端口 ${config.mixed_port}` : ""}
            </Card.Description>
          </Card.Header>
          <Card.Content>
            {running ? <Chip color="success">运行中</Chip> : <Chip color="danger">已停止</Chip>}
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>节点数量</Card.Title>
            <Card.Description>当前生效订阅的可用节点数</Card.Description>
          </Card.Header>
          <Card.Content>
            <span className="text-sm">{activeSub ? activeSub.node_count : "-"}</span>
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>规则数量</Card.Title>
            <Card.Description>本次合成配置的规则条数</Card.Description>
          </Card.Header>
          <Card.Content>
            <span className="text-sm">{status?.rule_count ?? 0}</span>
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>MITM 地址</Card.Title>
            <Card.Description>中间人代理监听地址</Card.Description>
          </Card.Header>
          <Card.Content>
            <span className="text-sm">{status?.mitm_addr ?? "未启用"}</span>
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>系统代理</Card.Title>
            <Card.Description>是否已接管系统代理</Card.Description>
          </Card.Header>
          <Card.Content>
            {status?.system_proxy ? <Chip color="success">已启用</Chip> : <Chip color="danger">未启用</Chip>}
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>Clash 面板</Card.Title>
            <Card.Description>面板 API 开启状态与访问入口</Card.Description>
          </Card.Header>
          <Card.Content className="flex flex-col gap-3">
            {!config?.clash_api_enabled ? (
              <div className="flex flex-col gap-1">
                <span className="text-sm">未启用</span>
                <span className="text-xs text-muted">在「设置 → Clash 面板」开启</span>
              </div>
            ) : (
              <>
                <div className="flex items-center gap-2">
                  {status?.clash_api_url && running ? (
                    <Chip color="success">运行中</Chip>
                  ) : (
                    <Chip color="danger">未运行</Chip>
                  )}
                </div>
                <span className="font-mono text-xs text-muted">http://127.0.0.1:{config.clash_api_port}/ui</span>
                <div className="flex items-center gap-2">
                  <Button size="sm" variant="secondary" onPress={() => void handleCopyLink()}>
                    {linkCopied ? "已复制" : "复制链接"}
                  </Button>
                  <Button size="sm" variant="secondary" onPress={() => void handleOpenPanel()}>
                    打开面板
                  </Button>
                </div>
              </>
            )}
          </Card.Content>
        </Card>
      </div>
    </div>
  );
}
