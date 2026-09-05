import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useAtom } from "jotai";
import { useCallback, useEffect, useRef, useState } from "react";
import { Alert, Button, Card, Label, ListBox, Select, Switch } from "@heroui/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  listCores,
  listProfiles,
  listSubscriptions,
  proxyStatus,
  refreshSubscription,
  requestVpnPermission,
  setActiveCore,
  setRuleMode as setRuleModeApi,
  startProxy,
  stopProxy,
  toErrorMessage,
  vpnLastError,
} from "../api";
import type {
  ClientConfig,
  ClientStatus,
  LocalCoreView,
  ProfileView,
  SubscriptionFormat,
  SubscriptionView,
} from "../api";
import { CORES_KEY, CONFIG_KEY, PROFILES_KEY, PROXY_STATUS_KEY, SUBSCRIPTIONS_KEY, VPN_ERROR_KEY } from "../api/keys";
import { lastActionErrorAtom } from "../atoms/ui";
import ConfigPreviewModal from "../components/ConfigPreviewModal";
import { useCapabilities } from "../hooks/useCapabilities";
import { useClientConfig, useSaveConfig } from "../hooks/useClientConfig";
import { useProxyStatus } from "../hooks/useProxyStatus";
import { toastError, toastSuccess, toastWarning } from "../toast";
import DashboardStatusCards, { coreLabel } from "./DashboardStatusCards";

/** 规则模式按钮（与后端 `rule` / `global` / `direct` 对齐）。 */
const RULE_MODES = [
  { id: "rule", label: "规则" },
  { id: "global", label: "全局" },
  { id: "direct", label: "直连" },
] as const;

/** 核心类型归一化（兼容 serde PascalCase `SingBox`/`Mihomo` 与小写 `singbox`/`mihomo`，未知回退 singbox）。 */
function coreTypeOf(value: string | undefined): "singbox" | "mihomo" {
  if (value === "Mihomo" || value === "mihomo") {
    return "mihomo";
  }
  return "singbox";
}

/** 由订阅 format 推导适配核心；ShareLinks/空 返回 null（跟随全局核心）。 */
function subCoreType(format: SubscriptionFormat | null | undefined): "singbox" | "mihomo" | null {
  if (format === "ClashYaml") {
    return "mihomo";
  }
  if (format === "SingBoxJson") {
    return "singbox";
  }
  return null;
}

/** 等待指定毫秒数（Android 启动确认轮询窗口用）。 */
function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export default function Dashboard() {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const { data: status } = useProxyStatus();
  const saveConfigMutation = useSaveConfig();
  // 保存进行中（替代原 store.loading；start/stop 在途由 busy 覆盖）。
  const loading = saveConfigMutation.isPending;
  // 跨页共享的最近操作错误（Alert 与 TUN/VPN 授权门禁消费）。
  const [error, setLastError] = useAtom(lastActionErrorAtom);
  const { data: capabilities } = useCapabilities();
  const [busy, setBusy] = useState<"start" | "stop" | null>(null);
  // 选中订阅后嗅探 format 期间置忙：禁用订阅 Select 避免重复触发。
  const [refreshingSub, setRefreshingSub] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [ruleModeBusy, setRuleModeBusy] = useState<string | null>(null);
  const [linkCopied, setLinkCopied] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [vpnAuthBusy, setVpnAuthBusy] = useState(false);
  // capabilities 异步返回前为 undefined，用 ref 让轮询读到最新平台。
  const capsRef = useRef(capabilities);
  useEffect(() => {
    capsRef.current = capabilities;
  }, [capabilities]);

  const isAndroid = capabilities?.is_android ?? false;

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

  // Android VPN 启动错误轮询（2s，与 proxy_status 轮询同频）。
  const { data: vpnErrorData } = useQuery<string | null>({
    queryKey: VPN_ERROR_KEY,
    queryFn: vpnLastError,
    enabled: isAndroid,
    refetchInterval: 2000,
    retry: false,
  });
  const vpnError = vpnErrorData ?? null;

  // ---- Mutations ----

  const selectSubMutation = useMutation({
    mutationFn: async ({
      id,
      needSniff,
      sub,
    }: {
      id: string;
      needSniff: boolean;
      sub: SubscriptionView | undefined;
    }) => {
      let derivedCore = sub ? subCoreType(sub.format) : null;

      if (needSniff) {
        setRefreshingSub(true);
        try {
          const sniffed = await refreshSubscription(id);
          derivedCore = subCoreType(sniffed.format);
          // 刷新成功：更新本地 query 缓存中的订阅数据
          queryClient.setQueryData<SubscriptionView[]>(SUBSCRIPTIONS_KEY, (prev) =>
            prev ? prev.map((item) => (item.id === sniffed.id ? sniffed : item)) : prev,
          );
        } catch {
          setRefreshingSub(false);
          await persistConfig({ active_subscription_id: id });
          toastWarning("订阅格式未知，未联动切换核心");
          await queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
          throw new Error("sniff_failed");
        }
        setRefreshingSub(false);
      }

      const patch: Partial<ClientConfig> = { active_subscription_id: id };
      if (derivedCore && derivedCore !== config?.core_type) {
        patch.core_type = derivedCore;
      }
      await persistConfig(patch);
      return { patch, needSniff, derivedCore };
    },
    onSuccess: ({ patch, needSniff, derivedCore }) => {
      setActionError(null);
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      if (patch.core_type) {
        toastWarning(`已按订阅格式切换至 ${coreLabel(patch.core_type)} 核心`);
      } else if (needSniff && derivedCore === null) {
        toastWarning("订阅格式未知，未联动切换核心");
      }
      void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
    },
    onError: (err: unknown) => {
      if (err instanceof Error && err.message === "sniff_failed") return;
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

  /**
   * Android 启动确认：Kotlin 核心在后台线程异步启动，`start_proxy` resolve 不代表启动成功
   * （真正失败仅写入 lastError）。每 500ms 轮询 `vpn_last_error` 与运行状态，3s 窗口内判定：
   * 有错误 → toastError（不显示成功）；running → toastSuccess；窗口耗尽 → toastWarning（不误报）。
   * 仅在 Android 分支调用（`vpn_last_error` 命令桌面不存在，调用会失败）。
   */
  const confirmAndroidStart = async () => {
    for (let attempt = 0; attempt < 6; attempt++) {
      await sleep(500);
      let lastError: string | null = null;
      try {
        // fetchQuery 拉取最新值并同步进缓存（供 VPN 启动失败 Alert 展示）。
        lastError = await queryClient.fetchQuery<string | null>({
          queryKey: VPN_ERROR_KEY,
          queryFn: vpnLastError,
          retry: false,
        });
      } catch {
        // 命令失败保持当前展示（非致命，避免轮询抖动）。
      }
      if (lastError) {
        // 手动更新 query 缓存以展示错误
        queryClient.setQueryData<string | null>(VPN_ERROR_KEY, lastError);
        toastError(lastError);
        await queryClient.invalidateQueries({ queryKey: PROXY_STATUS_KEY });
        return;
      }
      const status = await queryClient.fetchQuery<ClientStatus>({
        queryKey: PROXY_STATUS_KEY,
        queryFn: proxyStatus,
        retry: false,
      });
      if (status.core_running) {
        toastSuccess("代理已启动");
        return;
      }
    }
    toastWarning("代理正在后台启动…");
  };

  const handleStart = async () => {
    setBusy("start");
    // 新一次启动尝试先清掉上一次的失败展示（服务侧 lastError 成功启动后也会清空）。
    queryClient.setQueryData<string | null>(VPN_ERROR_KEY, null);
    try {
      await startMutation.mutateAsync();
      if (capsRef.current?.is_android) {
        // Android：核心异步启动，轮询确认后再提示，避免「启动失败却提示成功」。
        await confirmAndroidStart();
      } else {
        toastSuccess("代理已启动");
      }
    } catch (err) {
      // mutation onError 已记录共享错误（由页面 Alert 展示）；`tun_auth_required` / `vpn_not_authorized`
      // 走现有引导（TUN 授权页 / VPN 授权按钮）不重复 toast。
      const message = toErrorMessage(err);
      if (!message.includes("tun_auth_required") && !message.includes("vpn_not_authorized")) {
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
      if (!message.includes("tun_auth_required") && !message.includes("vpn_not_authorized")) {
        toastError(message);
      }
    }
    setBusy(null);
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
    }
    setVpnAuthBusy(false);
  };

  /** 选择生效订阅：订阅自身格式推导的核心与当前核心不一致时，同一次持久化联动切换。
   *  存量订阅未嗅探过 format（null/undefined）时先刷新拉取嗅探，拿到格式后再推导联动。 */
  const handleSelectSubscription = async (id: string) => {
    const sub = subs.find((item) => item.id === id);
    const needSniff = sub != null && sub.format == null;
    selectSubMutation.mutate({ id, needSniff, sub });
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
  // Android 下 start_proxy 未获 VPN 授权时返回 `vpn_not_authorized` 前缀错误，改为引导「去授权」。
  const vpnAuthRequired = error?.includes("vpn_not_authorized") ?? false;
  const alertError = error ?? actionError;

  // 运行门禁：不满足时禁止启动并逐条提示。
  const enabledSubs = subs.filter((sub) => sub.enabled);
  const activeSub = subs.find((sub) => sub.id === config?.active_subscription_id) ?? null;
  const activeCore = cores.find((core) => core.active) ?? null;

  // 桌面分支：核心二进制按当前 core_type 过滤（core_type 与二进制分属两个概念，
  // 切换核心类型后需重新选择匹配的二进制；active 二进制不属于当前类型时 Select
  // 显示 placeholder 不强行展示旧值，gate「请先选择要使用的核心」届时引导重新选择）。
  const desktopCores = cores.filter((core) => coreTypeOf(core.core_type) === coreTypeOf(config?.core_type));
  const desktopActiveCore = activeCore && desktopCores.includes(activeCore) ? activeCore : null;

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
  // 概念，核心类型由运行时自动判定），二进制门禁跳过。
  if (!isAndroid && (!config?.core_binary || !activeCore)) {
    gateMessages.push("请先选择要使用的核心");
  }
  // Android: clash 订阅与 sing-box 不兼容时自动降级到 mihomo（state.rs 中
  // check_subscription_core_compat 处理），前端不再显示格式不匹配门禁。
  if (!isAndroid) {
    if (activeSub && activeSub.format === "ClashYaml" && config?.core_type === "singbox") {
      gateMessages.push("该订阅为 Clash 格式，需切换 mihomo 核心");
    }
    if (activeSub && activeSub.format === "SingBoxJson" && config?.core_type === "mihomo") {
      gateMessages.push("该订阅为 sing-box 格式，需切换 sing-box 核心");
    }
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
            <Alert.Description className="break-all">{vpnError}</Alert.Description>
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
                  isDisabled={refreshingSub}
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
                // Phase ②: Android core selection silenced — hide switch UI,
                // show read-only status text (default sing-box, but
                // check_subscription_core_compat may auto-downgrade to mihomo).
                <div className="rounded-lg border border-border/60 bg-surface px-3 py-2 text-sm">
                  <span className="font-medium">自动（{coreLabel(config?.core_type ?? "singbox")}）</span>
                  <span className="ml-2 text-xs text-muted">（Android 内置核心）</span>
                </div>
              ) : (
                // 桌面分支：items 按当前 core_type 过滤；active 二进制不属于当前
                // 类型时显示 placeholder（不强行展示旧值），提示核心类型切换后需
                // 重新选择匹配的二进制。
                <Select
                  key="core-desktop"
                  id="dashboard-core"
                  aria-label="核心二进制"
                  value={desktopActiveCore?.path ?? ""}
                  onChange={(key) => void handleSelectCore(String(key ?? ""))}
                  placeholder="请选择核心"
                  isDisabled={desktopCores.length === 0}
                  fullWidth
                >
                  <Select.Trigger>
                    <Select.Value />
                    <Select.Indicator />
                  </Select.Trigger>
                  <Select.Popover>
                    <ListBox>
                      {desktopCores.length === 0 ? (
                        <ListBox.Item key="__empty" id="__empty" textValue="暂无可用核心">
                          暂无可用核心
                        </ListBox.Item>
                      ) : (
                        desktopCores.map((core) => (
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
      <DashboardStatusCards
        config={config}
        status={status}
        isAndroid={isAndroid}
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
