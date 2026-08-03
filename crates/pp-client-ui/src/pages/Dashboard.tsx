import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Chip, Label, ListBox, Select, Switch } from "@heroui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  listCores,
  listProfiles,
  listSubscriptions,
  platformInfo,
  requestVpnPermission,
  setActiveCore,
  setActiveSubscription,
  setRuleMode as setRuleModeApi,
  toErrorMessage,
  vpnLastError,
} from "../api";
import type { ClientConfig, CoreType, LocalCoreView, ProfileView, SubscriptionView } from "../api";
import ConfigPreviewModal from "../components/ConfigPreviewModal";
import { useAppStore } from "../store";
import { toastError, toastSuccess, toastWarning } from "../toast";

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

/** 核心类型原始值（兼容 `singbox` / `mihomo` 小写 serde 值与 PascalCase 值，回退 singbox）。 */
function coreTypeValue(value: string | undefined): string {
  if (value === "Mihomo") {
    return "mihomo";
  }
  if (value === "SingBox") {
    return "singbox";
  }
  return value ?? "singbox";
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
  const [previewOpen, setPreviewOpen] = useState(false);
  // 运行平台（Android 由 VpnService 接管，隐藏桌面专属开关）。
  const [os, setOs] = useState<string | null>(null);
  const [vpnAuthBusy, setVpnAuthBusy] = useState(false);
  // Android VPN 启动失败原因（非空时展示 danger Alert；来自 Kotlin ProxyVpnService.lastError）。
  const [vpnError, setVpnError] = useState<string | null>(null);
  // platformInfo 异步返回后才拿到 os，现有 2s 轮询闭包会捕获旧值；用 ref 让轮询读到最新平台。
  const osRef = useRef(os);
  osRef.current = os;

  /**
   * 运行配置开关的即时保存：从 store 取最新配置叠加补丁（避免闭包旧值）。
   * 保存结果通过全局 toast 反馈：有后端非阻塞提示走 warning，否则成功提示；
   * 失败时 store.error 由页面级 Alert 展示并 `loadConfig()` 回滚。
   */
  const persistConfig = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const current = useAppStore.getState().config;
      if (!current) {
        return;
      }
      try {
        const warning = await saveConfig({ ...current, ...patch });
        if (warning) {
          toastWarning(warning);
        } else {
          toastSuccess("设置已保存");
        }
      } catch (err) {
        // 保存失败仅落在 store.error（会被 Alert 展示但用户易忽略），补全局 toast 强化反馈。
        toastError(toErrorMessage(err));
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
    void platformInfo()
      .then((info) => setOs(info.os))
      .catch(() => {
        // 命令失败保持未知平台（按桌面渲染）。
      });
    // 状态轮询：每 2s 刷新一次运行状态；Android 并入读取 VPN 启动错误
    // （libbox 在后台线程启动，失败不阻塞 start_proxy 返回，需轮询兜底展示）。
    const timer = window.setInterval(() => {
      void refreshStatus();
      if (osRef.current === "android") {
        void vpnLastError()
          .then((err) => setVpnError(err ?? null))
          .catch(() => {
            // 命令失败保持当前展示（非致命，避免轮询抖动）。
          });
      }
    }, 2000);
    return () => window.clearInterval(timer);
  }, [loadConfig, refreshStatus, loadSubscriptions, loadCores, loadProfiles]);

  const handleStart = async () => {
    setBusy("start");
    // 新一次启动尝试先清掉上一次的失败展示（服务侧 lastError 成功启动后也会清空）。
    setVpnError(null);
    try {
      await start();
      toastSuccess("代理已启动");
    } catch (err) {
      // store 已记录 error（由页面 Alert 展示）；`tun_auth_required` / `vpn_not_authorized`
      // 走现有引导（TUN 授权页 / VPN 授权按钮）不重复 toast。
      const message = toErrorMessage(err);
      if (!message.includes("tun_auth_required") && !message.includes("vpn_not_authorized")) {
        toastError(message);
      }
    } finally {
      setBusy(null);
    }
  };

  const handleStop = async () => {
    setBusy("stop");
    try {
      await stop();
      toastSuccess("代理已停止");
    } catch (err) {
      const message = toErrorMessage(err);
      if (!message.includes("tun_auth_required") && !message.includes("vpn_not_authorized")) {
        toastError(message);
      }
    } finally {
      setBusy(null);
    }
  };

  /** 发起系统 VPN 授权（Android）：成功后引导重新启动代理。 */
  const handleVpnAuth = async () => {
    setVpnAuthBusy(true);
    try {
      await requestVpnPermission();
      setActionError(null);
      toastSuccess("VPN 授权成功，请重新启动代理");
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setVpnAuthBusy(false);
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
      // 同步 store.config，避免 persistConfig 用陈旧的 rule_mode 基底覆盖本次修改。
      await loadConfig();
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
  // Android 下 start_proxy 未获 VPN 授权时返回 `vpn_not_authorized` 前缀错误，改为引导「去授权」。
  const vpnAuthRequired = error?.includes("vpn_not_authorized") ?? false;
  const isAndroid = os === "android";
  const alertError = error ?? actionError;

  // 运行门禁：不满足时禁止启动并逐条提示。
  const enabledSubs = subs.filter((sub) => sub.enabled);
  const activeSub = subs.find((sub) => sub.id === config?.active_subscription_id) ?? null;
  const activeCore = cores.find((core) => core.active) ?? null;

  // 旧版 Hub 直连模式：未选择订阅但 hub_url 与 sub_token 均已配置时放行（deprecated）。
  const legacyHub = !activeSub && Boolean(config?.hub_url && config?.sub_token);

  // 配置预览门禁：需存在生效订阅（或旧版 Hub 直连配置），否则无可预览的合成配置。
  const canPreview = Boolean(activeSub || legacyHub);

  const gateMessages: string[] = [];
  if (!activeSub && !legacyHub) {
    gateMessages.push("请先选择要使用的订阅");
  }
  if (activeSub && !activeSub.enabled) {
    gateMessages.push("所选订阅已停用，请在订阅页启用或重新选择");
  }
  // Android 核心为内置双核心（sing-box libbox / mihomo wrapper，无「选择核心二进制」
  // 概念，核心类型在上方 Select 直接选择），二进制门禁跳过。
  if (!isAndroid && (!config?.core_binary || !activeCore)) {
    gateMessages.push("请先选择要使用的核心");
  }
  if (activeSub && activeSub.format === "ClashYaml" && config?.core_type === "singbox") {
    gateMessages.push("该订阅为 Clash 格式，需切换 mihomo 核心");
  }
  if (activeSub && activeSub.format === "SingBoxJson" && config?.core_type === "mihomo") {
    gateMessages.push("该订阅为 sing-box 格式，需切换 sing-box 核心");
  }
  if (activeSub?.profile_id) {
    const profile = profiles.find((p) => p.id === activeSub.profile_id);
    if (!profile) {
      gateMessages.push("关联的覆写模板已失效，请在订阅页重新关联");
    } else if (config && profile.core_type !== config.core_type) {
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

      {alertError && !tunAuthRequired && !vpnAuthRequired && (
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

      {vpnAuthRequired && (
        <Alert status="warning">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>需要 VPN 授权</Alert.Title>
            <Alert.Description>
              代理启动失败：Android 系统尚未授权本应用创建 VPN。点击「去授权」完成系统授权后重新启动代理。
            </Alert.Description>
            <div className="mt-2">
              <Button variant="secondary" size="sm" isPending={vpnAuthBusy} onPress={() => void handleVpnAuth()}>
                去授权
              </Button>
            </div>
          </Alert.Content>
        </Alert>
      )}

      {/* Android：libbox 后台启动失败被 start_proxy 静默吞掉，经 vpn_last_error() 轮询兜底展示。 */}
      {isAndroid && vpnError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>VPN 启动失败</Alert.Title>
            <Alert.Description>{vpnError}</Alert.Description>
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
              {isAndroid ? (
                // Android 核心为内置双核心（sing-box libbox / mihomo wrapper，经
                // panelcore.aar 合并绑定）：无「选择核心二进制」概念，改为直接选择
                // 核心类型（持久化 core_type，Kotlin 侧按 core 字段分派 VPN 服务）。
                <Select
                  id="dashboard-core"
                  value={coreTypeValue(config?.core_type)}
                  onChange={(key) => void persistConfig({ core_type: String(key ?? "singbox") })}
                  placeholder="请选择核心类型"
                  fullWidth
                >
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      <ListBox.Item id="singbox" textValue="sing-box">
                        sing-box
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                      <ListBox.Item id="mihomo" textValue="mihomo">
                        mihomo
                        <ListBox.ItemIndicator />
                      </ListBox.Item>
                    </ListBox>
                  </Select.Popover>
                </Select>
              ) : (
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
              )}
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

          {/* 系统代理 / MITM 为桌面专属开关：Android 由 VpnService 接管流量，
              两个开关无效故隐藏（保留运行配置其余部分）。 */}
          {!isAndroid && (
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
            </div>
          )}

          <div className="flex flex-wrap items-center gap-4">
            <Button variant="secondary" size="lg" isDisabled={!canPreview} onPress={() => setPreviewOpen(true)}>
              配置预览
            </Button>
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
                <span className="text-xs text-muted">
                  首次打开会自动下载面板资源，需网络可达；空白时检查网络或稍候重试
                </span>
              </>
            )}
          </Card.Content>
        </Card>
      </div>

      <ConfigPreviewModal isOpen={previewOpen} onClose={() => setPreviewOpen(false)} title="配置预览 — 当前生效配置" />
    </div>
  );
}
