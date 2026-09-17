import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon, PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card, Spinner } from "@heroui/react";
import {
  CONFIG_KEY,
  SUBSCRIPTIONS_KEY,
  addSubscription,
  listSubscriptions,
  refreshSubscription,
  removeSubscription,
  setActiveSubscription,
  setSubscriptionEnabled,
  toErrorMessage,
  toastError,
  toastSuccess,
  toastWarning,
  updateSubscription,
  useClientConfig,
} from "@pp/client-core";
import type { SubscriptionView } from "@pp/client-core";
import { SubPageShell } from "../components/SubPageShell";
import { SubscriptionDeleteConfirm } from "../components/SubscriptionDeleteConfirm";
import { SubscriptionFormSheet } from "../components/SubscriptionFormSheet";
import type { SubscriptionDraft } from "../components/SubscriptionFormSheet";
import { SubscriptionRow } from "../components/SubscriptionRow";

/**
 * 订阅管理页（ADR-0003 M5.6，路由 `/subscriptions`，二级页）。
 *
 * - 列表：卡片式——点击卡片设为生效（`setActiveSubscription`），右上 Switch 启停，
 *   底部刷新 / 编辑 / 删除；生效订阅高亮并显示「生效中」chip；
 * - 添加 / 编辑：底部 Sheet 表单（名称 + URL 必填），编辑预填；
 * - 删除：AlertDialog 确认 → `removeSubscription`；删除生效订阅时 toast
 *   「生效订阅已删除，请重新选择」（Rust 侧删除后 active 标记一并清除，需失效 CONFIG_KEY）；
 * - 刷新全部：列表顶部刷新图标，逐个 `refreshSubscription`，汇总成功/失败 toast；
 * - 数据经 @pp/client-core，mutation 成功后失效 SUBSCRIPTIONS_KEY（涉及生效标记/启停时
 *   同时失效 CONFIG_KEY），页面 5s 轮询对齐 desktop/首页。
 */
export default function Subscriptions() {
  const queryClient = useQueryClient();
  const { data: config } = useClientConfig();
  const activeId = config?.active_subscription_id ?? null;

  const {
    data: subscriptions = [],
    isLoading,
    error: queryError,
  } = useQuery<SubscriptionView[]>({
    queryKey: SUBSCRIPTIONS_KEY,
    queryFn: listSubscriptions,
    refetchInterval: 5000,
  });

  // ---- 局部 UI 状态 ----
  const [formOpen, setFormOpen] = useState(false);
  const [editingSub, setEditingSub] = useState<SubscriptionView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<SubscriptionView | null>(null);
  // 当前生效切换 / 启停的写操作按卡禁用（savingId 单飞，避免同卡并发写）。
  const [savingId, setSavingId] = useState<string | null>(null);
  // 单卡刷新中集合（支持跨卡并发刷新）。
  const [refreshingIds, setRefreshingIds] = useState<Set<string>>(new Set());
  const [refreshingAll, setRefreshingAll] = useState(false);

  const invalidateSubs = () => void queryClient.invalidateQueries({ queryKey: SUBSCRIPTIONS_KEY });
  const invalidateConfig = () => void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });

  // ---- Mutations ----
  const addMutation = useMutation({
    mutationFn: (draft: SubscriptionDraft) =>
      addSubscription({ name: draft.name, url: draft.url, user_agent: draft.userAgent || undefined }),
  });
  const editMutation = useMutation({
    mutationFn: ({ id, draft }: { id: string; draft: SubscriptionDraft }) =>
      updateSubscription(id, draft.name, draft.url, null, draft.userAgent || undefined),
  });
  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) => setSubscriptionEnabled(id, enabled),
  });
  const removeMutation = useMutation({
    mutationFn: (id: string) => removeSubscription(id),
  });

  const formBusy = addMutation.isPending || editMutation.isPending;
  const refreshAllDisabled = refreshingAll || refreshingIds.size > 0 || subscriptions.length === 0;

  // ---- 操作 ----
  const openAdd = () => {
    setEditingSub(null);
    setFormOpen(true);
  };

  const openEdit = (sub: SubscriptionView) => {
    setEditingSub(sub);
    setFormOpen(true);
  };

  const closeForm = () => {
    setFormOpen(false);
    setEditingSub(null);
  };

  const handleFormSave = async (draft: SubscriptionDraft) => {
    try {
      if (editingSub) {
        await editMutation.mutateAsync({ id: editingSub.id, draft });
        toastSuccess(`订阅「${editingSub.name}」已更新`);
      } else {
        const created = await addMutation.mutateAsync(draft);
        if (created.error) {
          toastWarning("订阅已添加，但首次拉取失败（列表内可查看原因并手动刷新）");
        } else {
          toastSuccess(`订阅「${created.name}」已添加 · ${created.node_count} 个节点`);
        }
      }
      closeForm();
      invalidateSubs();
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  const handleActivate = async (sub: SubscriptionView) => {
    if (savingId !== null || sub.id === activeId) return;
    if (!sub.enabled) {
      toastWarning("该订阅已停用，请先启用后再设为生效");
      return;
    }
    setSavingId(sub.id);
    try {
      await setActiveSubscription(sub.id);
      toastSuccess(`已将「${sub.name}」设为生效订阅`);
      invalidateConfig();
    } catch (err) {
      toastError(toErrorMessage(err));
    }
    setSavingId(null);
  };

  const handleToggle = async (sub: SubscriptionView) => {
    if (savingId !== null) return;
    const nextEnabled = !sub.enabled;
    const isActive = sub.id === activeId;
    setSavingId(sub.id);
    try {
      await toggleMutation.mutateAsync({ id: sub.id, enabled: nextEnabled });
      invalidateSubs();
      if (!nextEnabled && isActive) {
        // Rust 侧停用当前生效订阅时同步清空 active_subscription_id。
        toastWarning("生效订阅已停用，请重新选择生效订阅");
        invalidateConfig();
      } else {
        toastSuccess(nextEnabled ? `已启用「${sub.name}」` : `已停用「${sub.name}」`);
      }
    } catch (err) {
      toastError(toErrorMessage(err));
    }
    setSavingId(null);
  };

  const handleRefresh = async (sub: SubscriptionView) => {
    if (refreshingAll || refreshingIds.has(sub.id)) return;
    setRefreshingIds((prev) => new Set(prev).add(sub.id));
    try {
      const updated = await refreshSubscription(sub.id);
      if (updated.error) {
        toastWarning(`「${updated.name}」刷新失败，已保留上次数据`);
      } else {
        toastSuccess(`已刷新「${updated.name}」 · ${updated.node_count} 个节点`);
      }
      invalidateSubs();
    } catch (err) {
      toastError(toErrorMessage(err));
    }
    setRefreshingIds((prev) => {
      const next = new Set(prev);
      next.delete(sub.id);
      return next;
    });
  };

  const handleRefreshAll = async () => {
    if (refreshAllDisabled) return;
    setRefreshingAll(true);
    let ok = 0;
    let fail = 0;
    for (const sub of subscriptions) {
      try {
        const updated = await refreshSubscription(sub.id);
        if (updated.error) {
          fail += 1;
        } else {
          ok += 1;
        }
      } catch {
        fail += 1;
      }
    }
    invalidateSubs();
    setRefreshingAll(false);
    if (fail === 0) {
      toastSuccess(`已刷新全部 ${subscriptions.length} 个订阅`);
    } else {
      toastWarning(`刷新完成：${ok} 个成功，${fail} 个失败（请检查网络后重试）`);
    }
  };

  const handleDeleteConfirm = async () => {
    const sub = pendingDelete;
    if (!sub) return;
    const wasActive = sub.id === activeId;
    setPendingDelete(null);
    try {
      await removeMutation.mutateAsync(sub.id);
      invalidateSubs();
      invalidateConfig();
      if (wasActive) {
        toastWarning("生效订阅已删除，请重新选择");
      } else {
        toastSuccess(`已删除订阅「${sub.name}」`);
      }
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  const showList = !isLoading && !queryError && subscriptions.length > 0;

  return (
    <SubPageShell
      title="订阅管理"
      action={
        <Button variant="primary" className="h-11 shrink-0 px-4" onPress={openAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加
        </Button>
      }
    >
      {isLoading && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-muted">正在加载订阅…</span>
          </Card.Content>
        </Card>
      )}

      {!isLoading && queryError && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
            <span className="text-sm text-muted">订阅列表加载失败</span>
            <span className="text-xs text-muted/80">{toErrorMessage(queryError)}</span>
          </Card.Content>
        </Card>
      )}

      {!isLoading && !queryError && subscriptions.length === 0 && (
        <div className="flex flex-1 flex-col items-center justify-center gap-5 pb-16 text-center">
          <div className="flex flex-col gap-1">
            <span className="text-base font-medium text-foreground">添加你的第一个订阅</span>
            <span className="text-sm text-muted">粘贴机场订阅链接，拉取节点并用于首页启动代理</span>
          </div>
          <Button variant="primary" size="lg" className="min-h-12 px-8" onPress={openAdd}>
            添加订阅
          </Button>
        </div>
      )}

      {showList && (
        <>
          <div className="flex items-center justify-between">
            <span className="pl-1 text-xs text-muted">共 {subscriptions.length} 个订阅源</span>
            <button
              type="button"
              aria-label="刷新全部订阅"
              disabled={refreshAllDisabled}
              onClick={() => void handleRefreshAll()}
              className="flex size-11 shrink-0 items-center justify-center rounded-lg text-muted transition-opacity active:opacity-70 disabled:cursor-default disabled:opacity-40"
            >
              <ArrowPathIcon className={`size-5 ${refreshingAll ? "animate-spin" : ""}`} aria-hidden="true" />
            </button>
          </div>
          <div className="flex flex-col gap-2">
            {subscriptions.map((sub) => (
              <SubscriptionRow
                key={sub.id}
                sub={sub}
                isActive={sub.id === activeId}
                busy={savingId === sub.id || refreshingAll}
                refreshing={refreshingIds.has(sub.id)}
                onActivate={() => void handleActivate(sub)}
                onToggle={() => void handleToggle(sub)}
                onRefresh={() => void handleRefresh(sub)}
                onEdit={() => openEdit(sub)}
                onDelete={() => setPendingDelete(sub)}
              />
            ))}
          </div>
        </>
      )}

      <SubscriptionFormSheet
        isOpen={formOpen}
        editing={editingSub}
        busy={formBusy}
        onClose={closeForm}
        onSave={(draft) => void handleFormSave(draft)}
      />
      <SubscriptionDeleteConfirm
        sub={pendingDelete}
        isActive={pendingDelete?.id === activeId}
        busy={removeMutation.isPending}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => void handleDeleteConfirm()}
      />
    </SubPageShell>
  );
}
