import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { Alert, AlertDialog, Button } from "@heroui/react";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import {
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  isGroupOutbound,
  markRestartRequired,
  outboundTag,
  subscriptionNodeTags,
  subscriptionNodeTagsKey,
  toastError,
  toastSuccess,
  toErrorMessage,
  useClientConfig,
  useProxyStatus,
} from "@pp/client-core";
import type { ConfigSlices, CustomOutbound, NodeTagView, OutboundsSlice } from "@pp/client-core";
import {
  buildGroupMemberCandidates,
  isConfigSlices,
  isOutboundsSliceValid,
  validateOutboundsSlice,
  type GroupMemberCandidate,
  type OutboundProtocolType,
} from "@pp/client-core";
import { OutboundFormModal } from "./OutboundFormModal";
import { OutboundListSection } from "./OutboundListSection";

/**
 * 自定义出站切片配置页（桌面端，路由 `/config/outbounds`；语义对齐移动端
 * `Config/Outbounds/index.tsx`）。
 *
 * 出站可被「规则」页的 `Outbound{tag}` 动作引用（tag 由名称生成，强制 `slice-`
 * 前缀）。无切片总开关：启用中的条目即注入运行配置。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量切片；编辑只改内存草稿
 * （copy-on-write），保存才整份 `configSlicesSave` 落盘，成功后 invalidate + toast
 * 并上报重启脏标记；保存前整份草稿校验，失败禁用保存并在列表行内提示。
 */
export default function OutboundsPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 切片在核心启动时注入，运行中变更不热更新：保存成功上报全局脏标记。
  const coreRunning = status?.core_running ?? false;

  const {
    data: rawSlices,
    isLoading,
    error: queryError,
  } = useQuery<ConfigSlices>({
    queryKey: CONFIG_SLICES_KEY,
    queryFn: configSlicesGet,
    // 表单页草稿期间避免窗口聚焦触发的后台重取覆盖未保存编辑；保存后仍显式 invalidate。
    refetchOnWindowFocus: false,
  });
  // 分组成员候选数据源：静态订阅节点读生效订阅的本地缓存（不依赖核心运行）。
  const { data: config } = useClientConfig();
  const activeSubscriptionId = config?.active_subscription_id ?? null;
  const { data: subscriptionNodes } = useQuery<NodeTagView[]>({
    queryKey: subscriptionNodeTagsKey(activeSubscriptionId ?? ""),
    queryFn: () => subscriptionNodeTags(activeSubscriptionId ?? ""),
    enabled: !!activeSubscriptionId,
    retry: false,
  });
  // 生效订阅存在但缓存为空（从未同步 / 缓存丢失）：给「先同步订阅」引导文案。
  const subscriptionCacheAvailable = !!activeSubscriptionId && (subscriptionNodes?.length ?? 0) > 0;

  // 结构守卫：异构/异常缓存视为未加载，渲染空态而非崩溃。
  const slices = isConfigSlices(rawSlices) ? rawSlices : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });

  // ---- 内存草稿（query 数据变化时渲染期同步，copy-on-write 编辑） ----
  const [draft, setDraft] = useState<OutboundsSlice | null>(null);
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | null>(null);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    setDraft(slices.outbounds);
  }
  if (!slices && draft !== null) {
    setPrevSlices(null);
    setDraft(null);
  }

  // ---- 局部 UI 状态 ----
  const [formOpen, setFormOpen] = useState(false);
  const [editing, setEditing] = useState<CustomOutbound | null>(null);
  const [pendingDelete, setPendingDelete] = useState<CustomOutbound | null>(null);
  const [saving, setSaving] = useState(false);
  // 新建时预选协议（分组区 → selector，节点区 → vless）。
  const [newProtocol, setNewProtocol] = useState<OutboundProtocolType>("vless");

  const errors = draft ? validateOutboundsSlice(draft) : null;
  const valid = errors !== null && isOutboundsSliceValid(errors);
  const dirty = draft !== null && slices !== null && JSON.stringify(draft) !== JSON.stringify(slices.outbounds);
  const otherNames = useMemo(
    () => (draft ? draft.items.filter((item) => item.id !== editing?.id).map((item) => item.name) : []),
    [draft, editing],
  );

  // 内置 selector（proxy）的默认成员候选：运行时成员 = auto + 订阅节点。
  const builtinDefaultCandidates = useMemo<GroupMemberCandidate[]>(() => {
    const options: GroupMemberCandidate[] = [{ value: "auto", label: "auto（自动测速）", hint: "内置分组" }];
    for (const node of subscriptionNodes ?? []) {
      options.push({ value: node.tag, label: node.name, hint: "订阅节点" });
    }
    return options;
  }, [subscriptionNodes]);

  // 内置静态分组（global/final）的成员候选：内置 tag（排除自身；final 额外排除 global
  // 防循环）+ 订阅节点 + 切片节点。
  const builtinStaticMemberCandidates = useMemo<GroupMemberCandidate[]>(() => {
    const name = editing?.name ?? "";
    const builtinTags = ["proxy", "auto", "final", "global", "direct", "block"]
      .filter((tag) => tag !== name && !(name === "final" && tag === "global"))
      .map((tag) => ({ value: tag, label: tag, hint: "内置出站" }));
    const nodes = (subscriptionNodes ?? []).map((node) => ({
      value: node.tag,
      label: node.name,
      hint: "订阅节点",
    }));
    const sliceNodes = (draft?.items ?? [])
      .filter((item) => item.builtin !== true && item.enabled && !isGroupOutbound(item))
      .map((item) => ({ value: outboundTag(item.name), label: item.name, hint: "自定义出站" }));
    return [...builtinTags, ...nodes, ...sliceNodes];
  }, [editing?.name, subscriptionNodes, draft]);

  const memberCandidates = useMemo<GroupMemberCandidate[]>(() => {
    const sliceNodes = (draft?.items ?? [])
      .filter((item) => item.enabled && !isGroupOutbound(item))
      .map((item) => ({ name: item.name, tag: outboundTag(item.name) }));
    return buildGroupMemberCandidates({
      subscriptionNodes: (subscriptionNodes ?? []).map((node) => node.tag),
      sliceNodes,
    });
  }, [draft, subscriptionNodes]);

  const handleSave = async () => {
    if (!slices || !draft || !valid || !dirty || saving) return;
    setSaving(true);
    try {
      await configSlicesSave({ ...slices, outbounds: draft });
      markRestartRequired("outbounds", coreRunning);
      toastSuccess(coreRunning ? "自定义出站已保存，重启代理后生效" : "自定义出站已保存");
      await queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  // ---- 出站条目 ----
  const openAdd = (protocol: OutboundProtocolType) => {
    setNewProtocol(protocol);
    setEditing(null);
    setFormOpen(true);
  };

  const openEdit = (item: CustomOutbound) => {
    setEditing(item);
    setFormOpen(true);
  };

  const handleSaveItem = (item: CustomOutbound) => {
    setDraft((current) => {
      if (!current) return current;
      const exists = current.items.some((existing) => existing.id === item.id);
      return exists
        ? { ...current, items: current.items.map((existing) => (existing.id === item.id ? item : existing)) }
        : { ...current, items: [...current.items, item] };
    });
  };

  const handleToggleItem = (item: CustomOutbound, next: boolean) => {
    setDraft((current) =>
      current
        ? {
            ...current,
            items: current.items.map((existing) =>
              existing.id === item.id ? { ...existing, enabled: next } : existing,
            ),
          }
        : current,
    );
  };

  const handleDeleteRequest = (item: CustomOutbound) => {
    setFormOpen(false);
    setPendingDelete(item);
  };

  const handleDeleteConfirm = () => {
    const target = pendingDelete;
    setPendingDelete(null);
    if (!target) return;
    setDraft((current) =>
      current ? { ...current, items: current.items.filter((item) => item.id !== target.id) } : current,
    );
  };

  return (
    <div className="flex max-w-2xl flex-col gap-6">
      {/* 页头：返回 + 标题 + 保存 */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" isIconOnly aria-label="返回配置管理" onPress={() => navigate("/config")}>
            <ArrowLeftIcon className="size-4" />
          </Button>
          <div>
            <h1 className="text-xl font-semibold">出站管理</h1>
            <p className="text-sm text-muted">自定义代理节点与分组</p>
          </div>
        </div>
        <Button
          size="sm"
          variant="primary"
          isDisabled={!valid || !dirty || saving}
          isPending={saving}
          onPress={() => void handleSave()}
        >
          {dirty ? "保存" : "已保存"}
        </Button>
      </div>

      {isLoading && !slices && (
        <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
          <span className="text-sm text-muted">正在加载自定义出站配置…</span>
        </div>
      )}

      {!isLoading && queryError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>加载失败</Alert.Title>
            <Alert.Description>{toErrorMessage(queryError)}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {!isLoading && !queryError && !slices && (
        <div className="flex flex-col items-center gap-3 rounded-lg border border-border/60 bg-surface p-6 text-center">
          <span className="text-sm text-muted">自定义出站配置不可用</span>
          <Button size="sm" variant="secondary" onPress={invalidate}>
            重新加载
          </Button>
        </div>
      )}

      {draft && (
        <OutboundListSection
          items={draft.items}
          itemErrors={errors?.itemErrors ?? []}
          onToggle={handleToggleItem}
          onEdit={openEdit}
          onAddGroup={() => openAdd("selector")}
          onAddNode={() => openAdd("vless")}
        />
      )}

      {/* 编辑弹窗与删除确认（常驻挂载，isOpen / 目标控制显隐） */}
      <OutboundFormModal
        isOpen={formOpen}
        editing={editing}
        otherNames={otherNames}
        defaultProtocol={newProtocol}
        memberCandidates={
          editing?.builtin === true
            ? editing.name === "global" || editing.name === "final"
              ? builtinStaticMemberCandidates
              : builtinDefaultCandidates
            : memberCandidates
        }
        builtinMembersEditable={editing?.builtin === true && (editing.name === "global" || editing.name === "final")}
        subscriptionCacheAvailable={subscriptionCacheAvailable}
        onClose={() => setFormOpen(false)}
        onSave={handleSaveItem}
        onDeleteRequest={handleDeleteRequest}
      />

      <AlertDialog.Backdrop
        isOpen={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除自定义出站</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">确定删除出站「{pendingDelete?.name}」吗？该操作不可撤销。</p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={handleDeleteConfirm}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
