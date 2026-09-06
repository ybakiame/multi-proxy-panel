import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useAtom } from "jotai";
import { useCallback, useState } from "react";
import { Alert, Button, Card, Label, ListBox, Select, Switch } from "@heroui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listCores, listProfiles, listSubscriptions } from "@pp/client-core";
import { setActiveCore, setRuleMode as setRuleModeApi, startProxy, stopProxy, toErrorMessage } from "@pp/client-core";
import { CORES_KEY, CONFIG_KEY, PROFILES_KEY, PROXY_STATUS_KEY } from "@pp/client-core";
import { SUBSCRIPTIONS_KEY, lastActionErrorAtom } from "@pp/client-core";
import type { ClientConfig, ClientStatus, LocalCoreView, ProfileView, SubscriptionView } from "@pp/client-core";
import { useCapabilities, useClientConfig, useProxyStatus, useSaveConfig } from "@pp/client-core";
import { toastError, toastSuccess, toastWarning } from "@pp/client-core";
import ConfigPreviewModal from "../components/ConfigPreviewModal";
import DashboardStatusCards from "./DashboardStatusCards";

/** 规则模式按钮（与后端 `rule` / `global` / `direct` 对齐）。 */
const RULE_MODES = [
  { id: "rule", label: "规则" },
  { id: "global", label: "全局" },
  { id: "direct", label: "直连" },
] as const;

export default function Dashboard() {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const { data: status } = useProxyStatus();
  const saveConfigMutation = useSaveConfig();
  // 保存进行中（替代原 store.loading；start/stop 在途由 busy 覆盖）。
  const loading = saveConfigMutation.isPending;
  // 跨页共享的最近操作错误（Alert 与 TUN 授权门禁消费）。
  const [error, setLastError] = useAtom(lastActionErrorAtom);
  const { data: capabilities } = useCapabilities();
  const [busy, setBusy] = useState<"start" | "stop" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [ruleModeBusy, setRuleModeBusy] = useState<string | null>(null);
  const [linkCopied, setLinkCopied] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);

  // ---- TanStack Query: data fetching ----

  const { data: subs = [] } = useQuery<SubscriptionView[]>({
    queryKey: SUBSCRIPTIONS_KEY,
    queryFn: listSubscriptions,
    refetchInterval: 5000,
  });

  const coreMgmt = capabilities?.capabilities.core_management ?? false;

  const { data: cores = [] } = useQuery<LocalCoreView[]>({
    queryKey: CORES_KEY,
    queryFn: listCores,
    enabled: coreMgmt,
  });

  const { data: profiles = [] } = useQuery<ProfileView[]>({
    queryKey: PROFILES_KEY,
    queryFn: listProfiles,
  });

  // ---- Mutations ----

  const selectSubMutation = useMutation({
    mutationFn: async (id: string) => {
      await persistConfig({ active_subscription_id: id });
    },
    onSuccess: () => {
      setActionError(null);
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      setActionError(toErrorMessage(err));
    },
  });

  const selectCoreMutation = useMutation({
    mutationFn: async (path: string) => {
      await setActiveCore(path);
    },
    onSuccess: () => {
      setActionError(null);
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      void queryClient.invalidateQueries({ queryKey: CORES_KEY });
    },
    onError: (err: unknown) => {
      setActionError(toErrorMessage(err));
    },
  });

  const ruleModeMutation = useMutation({
    mutationFn: async (mode: string) => {
      const next = await setRuleModeApi(mode);
      return next;
    },
    onSuccess: (next) => {
      // set_rule_mode 返回最新运行状态，直接回写缓存（不触发重读）。
      queryClient.setQueryData(PROXY_STATUS_KEY, next);
      setActionError(null);
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
    },
    onError: (err: unknown) => {
      setActionError(toErrorMessage(err));
    },
  });

  /**
   * 运行配置开关的即时保存：从 Query 缓存取最新配置叠加补丁（避免闭包旧值）。
   * 保存结果通过全局 toast 反馈：有后端非阻塞提示走 warning，否则成功提示；
   * 失败时 lastActionErrorAtom 由页面级 Alert 展示并失效 CONFIG_KEY 重读回滚。
   */
  const persistConfig = useCallback(
    async (patch: Partial<ClientConfig>) => {
      const current = queryClient.getQueryData<ClientConfig>(CONFIG_KEY);
      if (!current) {
        return;
      }
      try {
        const { warning } = await saveConfigMutation.mutateAsync({ ...current, ...patch });
        if (warning) {
          toastWarning(warning);
        } else {
          toastSuccess("设置已保存");
        }
      } catch (err) {
        // 保存失败仅落在 lastActionErrorAtom（会被 Alert 展示但用户易忽略），补全局 toast 强化反馈。
        toastError(toErrorMessage(err));
        await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      }
    },
    [queryClient, saveConfigMutation],
  );

  // start/stop 成功后用返回的最新状态回写缓存；失败写入共享错误（Alert / 授权门禁消费）。
  const startMutation = useMutation({
    mutationFn: startProxy,
    onSuccess: (next: ClientStatus) => {
      queryClient.setQueryData(PROXY_STATUS_KEY, next);
      setLastError(null);
    },
    onError: (err: unknown) => {
      setLastError(toErrorMessage(err));
    },
  });

  const stopMutation = useMutation({
    mutationFn: stopProxy,
    onSuccess: (next: ClientStatus) => {
      queryClient.setQueryData(PROXY_STATUS_KEY, next);
      setLastError(null);
    },
    onError: (err: unknown) => {
      setLastError(toErrorMessage(err));
    },
  });

  const handleStart = async () => {
    setBusy("start");
    try {
      await startMutation.mutateAsync();
      toastSuccess("代理已启动");
    } catch (err) {
      // mutation onError 已记录共享错误（由页面 Alert 展示）；`tun_auth_required`
      // 走现有引导（TUN 授权提示）不重复 toast。
      const message = toErrorMessage(err);
      if (!message.includes("tun_auth_required")) {
        toastError(message);
      }
    }
    setBusy(null);
  };

  const handleStop = async () => {
    setBusy("stop");
    try {
      await stopMutation.mutateAsync();
      toastSuccess("代理已停止");
    } catch (err) {
      const message = toErrorMessage(err);
      if (!message.includes("tun_auth_required")) {
        toastError(message);
      }
    }
    setBusy(null);
  };

  /** 选择生效订阅：仅持久化 active_subscription_id（单核心，无格式联动）。 */
  const handleSelectSubscription = async (id: string) => {
    selectSubMutation.mutate(id);
  };

  const handleSelectCore = async (path: string) => {
    selectCoreMutation.mutate(path);
  };

  const handleRuleMode = async (mode: string) => {
    setRuleModeBusy(mode);
    ruleModeMutation.mutate(mode, {
      onSettled: () => setRuleModeBusy(null),
    });
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
  if (!config?.core_binary || !activeCore) {
    gateMessages.push("请先选择要使用的核心");
  }
  // 单核心（sing-box）：ClashYaml 订阅由 Rust 侧转换为 sing-box 运行，无格式门禁。
  if (activeSub?.profile_id) {
    const profile = profiles.find((p) => p.id === activeSub.profile_id);
    if (!profile) {
      gateMessages.push("关联的覆写模板已失效，请在订阅页重新关联");
    }
  }
  const canStart = gateMessages.length === 0;

  // 规则模式：优先取运行状态，其次配置，默认 rule。
  const ruleMode = status?.rule_mode ?? config?.rule_mode ?? "rule";
  const ruleModeHint =
    running && config?.clash_api_enabled
      ? "即时生效"
      : "已保存，将在下次启动生效（sing-box 运行时切换依赖 Clash 面板 API，需在「设置 → Clash 面板」开启）";

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
                  aria-label="生效订阅"
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
              {/* 桌面：单核心（sing-box），列出全部可用二进制。 */}
              <Select
                key="core-desktop"
                id="dashboard-core"
                aria-label="核心二进制"
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
                      <ListBox.Item key="__empty" id="__empty" textValue="暂无可用核心">
                        暂无可用核心
                      </ListBox.Item>
                    ) : (
                      cores.map((core) => (
                        <ListBox.Item key={core.path} id={core.path} textValue={`sing-box ${core.version}`}>
                          sing-box {core.version}
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
          {legacyHub && (
            <span className="text-xs text-warning">使用旧版 Hub 订阅（deprecated），建议到「订阅」页添加订阅</span>
          )}

          {/* 系统代理 / MITM 开关（桌面由核心直接接管系统流量）。 */}
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
      <DashboardStatusCards
        config={config}
        status={status}
        running={running}
        activeSub={activeSub}
        linkCopied={linkCopied}
        onCopyLink={() => void handleCopyLink()}
        onOpenPanel={() => void handleOpenPanel()}
      />

      <ConfigPreviewModal isOpen={previewOpen} onClose={() => setPreviewOpen(false)} title="配置预览 — 当前生效配置" />
    </div>
  );
}
