import { useMutation, useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { ChevronDownIcon } from "@heroicons/react/24/outline";
import { InlineAlert } from "@pp/ui";
import { Button } from "@pp/ui";
import {
  VPN_ERROR_KEY,
  requestVpnPermission,
  toErrorMessage,
  toastError,
  toastSuccess,
  vpnLastError,
} from "@pp/client-core";
import { useDashboardState } from "@pp/client-core";
import { CurrentNodeCard } from "../../components/mobile/CurrentNodeCard";
import { PageShell } from "../../components/PageShell";
import { StartStopFab } from "../../components/mobile/StartStopFab";
import { StatusCard } from "../../components/mobile/StatusCard";
import { SubscriptionSheet } from "../../components/mobile/SubscriptionSheet";
import { TodayStatsCard } from "../../components/TodayStatsCard";
import { TrafficCard } from "../../components/mobile/TrafficCard";

/** start_proxy 未获系统 VPN 授权时的错误前缀（Kotlin `vpn_not_authorized` reject，与 desktop 识别一致）。 */
const VPN_AUTH_MARKER = "vpn_not_authorized";

/**
 * 首页（仪表盘，ADR-0003 M5）。区块自上而下：
 *
 * 1. 头部：应用名 + 生效订阅行（点击开 SubscriptionSheet；无生效订阅时行下提示先选择）；
 * 2. 状态卡（StatusCard）：运行状态大字；运行中内嵌出站模式分段切换（`RuleModeSwitch`），
 *    未运行时仅展示状态并保留已保存模式 chip；
 * 3. 今日流量卡（TodayStatsCard）：本地持久化统计（已代理流量主指标），
 *    未运行时也展示，点击进入 `/stats` 详情页；
 * 4. 运行依赖区（仅核心运行中渲染，未运行时整组隐藏，相关轮询亦不发起）：
 *    当前节点卡（CurrentNodeCard，订阅名/分组/当前节点，点击进面板页）；
 * 5. 主操作：右下角悬浮启停按钮（StartStopFab，无生效订阅时禁用并提示选择）；点「启动代理」
 *    即完成「启动 →（遇 `vpn_not_authorized`）自动请求 VPN 授权 → 授权成功自动重试启动」
 *    的一次点击链路；授权被拒 / 重试仍失败则落错误展示（含「去授权」兜底按钮）；
 * 6. VPN 授权引导与启动失败错误：均为独立 InlineAlert 卡片，不再包裹启停按钮。
 *
 * 配置预览等开发者入口已迁移至设置页「开发者工具」分组。
 */
export default function Dashboard() {
  // ---- 局部 UI 状态 ----
  const [sheetOpen, setSheetOpen] = useState(false);
  // 最近启停/授权的同步错误（展示于主操作卡片内）；`vpn_not_authorized` 由下方
  // 「需要 VPN 授权」引导接管，自动授权/重试链路期间主按钮呈 busy 态。
  const [actionError, setActionError] = useState<string | null>(null);

  const {
    queryClient,
    config,
    status,
    capabilities,
    subscriptions,
    activeSub,
    running,
    ruleMode,
    startMutation,
    stopMutation,
  } = useDashboardState({ onActionError: setActionError, onActionSuccess: () => setActionError(null) });
  const isAndroid = capabilities?.is_android ?? false;
  const canStart = activeSub !== null;

  // Android 后台启动失败兜底：2s 轮询 vpn_last_error（与 proxy_status 同频）。
  const { data: vpnErrorData } = useQuery<string | null>({
    queryKey: VPN_ERROR_KEY,
    queryFn: vpnLastError,
    enabled: isAndroid,
    refetchInterval: 2000,
    retry: false,
  });
  const vpnError = vpnErrorData ?? null;

  const reportError = (err: unknown) => setActionError(toErrorMessage(err));
  // 系统 VPN 授权（request_vpn_permission → VpnService.prepare）。
  const vpnAuthMutation = useMutation({
    mutationFn: requestVpnPermission,
    onSuccess: () => {
      // 授权成功：清空失败展示，后续由调用方（自动链路 / 兜底按钮）重试启动。
      setActionError(null);
      queryClient.setQueryData<string | null>(VPN_ERROR_KEY, null);
    },
    onError: reportError,
  });

  const isVpnAuthError = (err: unknown) => toErrorMessage(err).includes(VPN_AUTH_MARKER);

  /**
   * 启动代理一次。成功回写状态 + toast；`vpn_not_authorized` 返回 "auth-needed"
   * 交授权链路处理（不 toast），其余失败 toast + actionError 展示。
   */
  const attemptStart = async (): Promise<"ok" | "auth-needed" | "failed"> => {
    try {
      await startMutation.mutateAsync();
      toastSuccess("代理已启动");
      return "ok";
    } catch (err) {
      if (isVpnAuthError(err)) {
        return "auth-needed";
      }
      toastError(toErrorMessage(err));
      return "failed";
    }
  };

  /** 请求系统 VPN 授权；授权成功后自动重试启动一次（handleStart 与「去授权」兜底按钮共用）。 */
  const authorizeAndStart = async () => {
    try {
      await vpnAuthMutation.mutateAsync();
    } catch (err) {
      // 授权被拒/取消：mutation onError 已把错误写入 actionError（含 vpn_not_authorized），
      // 卡内显示「需要 VPN 授权」引导并保留「去授权」按钮供用户改变主意后再发起。
      reportError(err);
      return;
    }
    // 授权成功 → 自动重试启动一次（仅一次，防循环）；仍失败走下方错误展示（含 vpnLastError 轮询兜底）。
    await attemptStart();
  };

  const handleStart = async () => {
    if (startMutation.isPending || vpnAuthMutation.isPending || stopMutation.isPending) {
      return;
    }
    // 新一轮启动先清空上轮失败展示（服务侧成功启动后也会清空 vpn_last_error）。
    setActionError(null);
    queryClient.setQueryData<string | null>(VPN_ERROR_KEY, null);
    if ((await attemptStart()) === "auth-needed") {
      // 未获系统 VPN 授权 → 自动拉起系统授权弹窗，授权成功后在同一链路内重试启动。
      await authorizeAndStart();
    }
  };

  const handleStop = async () => {
    try {
      await stopMutation.mutateAsync();
      toastSuccess("代理已停止");
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  // 错误含 `vpn_not_authorized` → 仅显示「需要 VPN 授权」引导（对齐 desktop Dashboard）。
  const message = actionError ?? "";
  const vpnAuthRequired = message.includes(VPN_AUTH_MARKER) || (vpnError ?? "").includes(VPN_AUTH_MARKER);
  const showActionError = message !== "" && !vpnAuthRequired;
  const showVpnError = vpnError !== null && !vpnAuthRequired;
  // 主操作 busy：覆盖「启动→授权→重试」整条链路（授权弹窗停留期间按钮禁用）。
  const startingPending = startMutation.isPending || vpnAuthMutation.isPending;

  return (
    <PageShell>
      {/* 1. 头部：应用名 + 生效订阅行 */}
      <header className="flex flex-col gap-3">
        <div>
          <h1 className="text-xl font-semibold">ProxyPanel</h1>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">仪表盘 · 核心启停与运行状态</p>
        </div>
        <Button
          variant="secondary"
          className="h-12 w-full justify-between px-4 font-normal"
          onPress={() => setSheetOpen(true)}
        >
          <span className="text-sm text-zinc-500 dark:text-zinc-400">生效订阅</span>
          <span className="flex min-w-0 items-center gap-1 text-sm font-medium text-zinc-900 dark:text-zinc-100">
            <span className="truncate">
              {activeSub ? `${activeSub.name} · ${activeSub.node_count} 节点` : "请选择订阅"}
            </span>
            <ChevronDownIcon className="size-4 shrink-0 text-zinc-400" aria-hidden="true" />
          </span>
        </Button>
        {!running && !canStart && <p className="text-xs text-amber-500">请先选择要使用的订阅</p>}
      </header>

      {/* 2. 状态卡（运行中内嵌出站模式分段切换） */}
      <StatusCard
        running={running}
        ruleMode={ruleMode}
        clashApiEnabled={config?.clash_api_enabled ?? false}
        missingRuleSets={status?.missing_rule_sets ?? []}
      />

      {/* 3. 今日流量（持久化统计，未运行时也展示；点击进入详情页） */}
      <TodayStatsCard running={running} />

      {/* 4. 运行依赖区（仅核心运行中渲染；未运行时整组隐藏，相关轮询不发起） */}
      {running && (
        <>
          {/* 当前节点卡（订阅名/分组/当前节点，点击进入面板页） */}
          <CurrentNodeCard running={running} subscriptionName={activeSub?.name ?? null} />

          {/* 流量统计卡（Clash API 会话实时统计） */}
          <TrafficCard running={running} clashApiUrl={status?.clash_api_url ?? null} />
        </>
      )}

      {/* 5. VPN 授权引导 / 启动失败：独立条件卡片，不再包裹启停按钮 */}
      {/* 授权被拒 / 重试仍遇未授权：显示引导 + 「去授权」兜底按钮；授权链路进行中隐藏避免与系统弹窗重叠 */}
      {vpnAuthRequired && !startingPending && (
        <InlineAlert kind="warning" title="需要 VPN 授权">
          启动代理需要 Android 系统授权创建
          VPN。授权被拒绝或取消时无法启动，点击「去授权」重新发起，授权成功后将在同一链路内自动启动代理。
          <div className="mt-3">
            <Button
              variant="secondary"
              size="lg"
              className="min-h-11"
              isPending={vpnAuthMutation.isPending}
              onPress={() => void authorizeAndStart()}
            >
              去授权
            </Button>
          </div>
        </InlineAlert>
      )}

      {/* 启动失败：同步错误文本 + vpn_last_error（若有） */}
      {(showActionError || showVpnError) && !startingPending && (
        <InlineAlert kind="danger" title="启动失败">
          {showActionError && <div className="break-all">{actionError}</div>}
          {showVpnError && <div className="break-all">{vpnError}</div>}
        </InlineAlert>
      )}

      {/* 底部留白：为悬浮启停按钮（FAB）让出空间，避免遮挡最后一张卡片 */}
      <div className="h-16 shrink-0" aria-hidden="true" />

      {/* 6. 悬浮启停按钮（FAB，右下角悬浮于 TabBar 之上） */}
      <StartStopFab
        running={running}
        starting={startingPending}
        stopping={stopMutation.isPending}
        canStart={canStart}
        onStart={handleStart}
        onStop={handleStop}
      />

      {/* 订阅切换 Sheet */}
      <SubscriptionSheet
        isOpen={sheetOpen}
        onClose={() => setSheetOpen(false)}
        subscriptions={subscriptions}
        activeSubscriptionId={config?.active_subscription_id ?? null}
      />
    </PageShell>
  );
}
