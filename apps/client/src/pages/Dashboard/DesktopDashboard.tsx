import { useMutation, useQuery } from "@tanstack/react-query";
import { useAtom } from "jotai";
import { useCallback, useState } from "react";
import { Alert, Card } from "@pp/ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import { listCores, listProfiles } from "@pp/client-core";
import { activateSubscription, setActiveCore, toErrorMessage } from "@pp/client-core";
import { CORES_KEY, CONFIG_KEY, PROFILES_KEY } from "@pp/client-core";
import { SUBSCRIPTIONS_KEY, lastActionErrorAtom } from "@pp/client-core";
import type { ClientConfig, LocalCoreView, ProfileView } from "@pp/client-core";
import { useSaveConfig } from "@pp/client-core";
import { toastError, toastSuccess, toastWarning } from "@pp/client-core";
import { markRestartRequired } from "@pp/client-core";
import ConfigPreviewModal from "../../components/desktop/ConfigPreviewModal";
import { TodayStatsCard } from "../../components/TodayStatsCard";
import { useDashboardState } from "@pp/client-core";
import { RuleModeSwitch } from "../../components/RuleModeSwitch";
import { DesktopRunConfig } from "./DesktopRunConfig";
import DashboardStatusCards from "../DashboardStatusCards";

export default function Dashboard() {
  const saveConfigMutation = useSaveConfig();
  // 保存进行中（替代原 store.loading；start/stop 在途由 busy 覆盖）。
  const loading = saveConfigMutation.isPending;
  // 跨页共享的最近操作错误（Alert 与 TUN 授权门禁消费）。
  const [error, setLastError] = useAtom(lastActionErrorAtom);
  const {
    queryClient,
    config,
    status,
    capabilities,
    subscriptions: subs,
    activeSub,
    running,
    ruleMode,
    startMutation,
    stopMutation,
  } = useDashboardState({ onActionError: setLastError, onActionSuccess: () => setLastError(null) });
  const [busy, setBusy] = useState<"start" | "stop" | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [linkCopied, setLinkCopied] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);

  // ---- TanStack Query: data fetching ----

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
      const sub = subs.find((item) => item.id === id);
      if (!sub) throw new Error("订阅不存在，请刷新后重试");
      await activateSubscription(sub);
    },
    onSuccess: () => {
      setActionError(null);
      // 生效订阅切换需重启核心才生效：上报全局脏标记（RestartPrompt 消费）。
      markRestartRequired("subscription", status?.core_running ?? false);
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

  // start_proxy 在 TUN 未授权时返回 `tun_auth_required` 错误，改为引导前往入站管理页授权。
  // 共享错误为会话级状态：TUN 此前开启时启动失败留下的错误可能残留——桌面端 TUN 为可选
  // 模式，仅当配置中 TUN 仍处于启用状态时才展示授权门禁（已关闭则按普通错误提示，避免
  // 「未启用 TUN 却被要求授权」的误导）。
  const tunAuthRequired = (config?.tun_enabled ?? false) && (error?.includes("tun_auth_required") ?? false);
  const alertError = error ?? actionError;

  // 运行门禁：不满足时禁止启动并逐条提示。
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
    gateMessages.push("请重新选择生效订阅");
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
              代理启动失败：TUN 模式未获得系统授权。请前往「配置 → 入站管理 → TUN
              入站」点击「立即授权」后重新启动代理；如不使用 TUN，可在该页关闭「启用 TUN 模式」。
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      <DesktopRunConfig
        config={config}
        subscriptions={subs}
        cores={cores}
        activeCore={activeCore}
        gateMessages={gateMessages}
        legacyHub={legacyHub}
        canPreview={canPreview}
        canStart={canStart}
        running={running}
        busy={busy}
        loading={loading}
        onSelectSubscription={handleSelectSubscription}
        onSelectCore={handleSelectCore}
        onPersist={persistConfig}
        onPreview={() => setPreviewOpen(true)}
        onStart={handleStart}
        onStop={handleStop}
      />

      {/* B. 规则模式 */}
      <Card>
        <Card.Header>
          <Card.Title>规则模式</Card.Title>
          <Card.Description>切换流量路由策略</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-3">
          <RuleModeSwitch
            value={ruleMode}
            running={running}
            clashApiEnabled={config?.clash_api_enabled ?? false}
            onError={setActionError}
            onSuccess={() => setActionError(null)}
          />
        </Card.Content>
      </Card>

      {/* C. 今日流量（点击进入统计详情页） */}
      <TodayStatsCard running={running} />

      {/* D. 状态卡片 */}
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
