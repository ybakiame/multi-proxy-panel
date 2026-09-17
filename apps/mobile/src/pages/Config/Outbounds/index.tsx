import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  isGroupOutbound,
  outboundTag,
  subscriptionNodeTags,
  subscriptionNodeTagsKey,
  toErrorMessage,
  toastError,
  toastSuccess,
  useClientConfig,
  useProxyStatus,
} from "@pp/client-core";
import type { ConfigSlices, CustomOutbound, NodeTagView, OutboundsSlice } from "@pp/client-core";
import { BackHeader } from "../../../components/BackHeader";
import { OutboundDeleteConfirm } from "./OutboundDeleteConfirm";
import { OutboundFormSheet } from "./OutboundFormSheet";
import { OutboundListSection } from "./OutboundListSection";
import { buildGroupMemberCandidates, type GroupMemberCandidate } from "./groupForm";
import { isConfigSlices, isOutboundsSliceValid, validateOutboundsSlice } from "./outboundForm";
import type { OutboundProtocolType } from "./outboundOptions";

/**
 * 自定义出站切片配置子页（ADR-0005 P0-4c，路由 `/config/outbounds`）。
 *
 * 结构自上而下：BackHeader（右侧保存动作）→ 出站列表（增删改）。
 * 出站可被「规则管理」的 `Outbound{tag}` 动作引用（tag 由名称生成，强制 `slice-` 前缀）。
 * 无切片总开关：启用中的条目即注入运行配置。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量 `ConfigSlices`；所有编辑只改内存中的
 * outbounds 切片草稿（copy-on-write），点击保存才整份 `configSlicesSave` 落盘，成功后
 * invalidate + toast；核心运行中追加「重启代理后生效」。校验（D5）在保存前对整份草稿
 * 执行，失败禁用保存并在列表行内提示。
 */
export default function OutboundsPage() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 切片在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
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
  // 分组成员候选数据源：静态订阅节点读生效订阅的本地缓存（不依赖核心运行），
  // 无生效订阅时不发起；核心未运行时仍能拿到订阅节点。
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
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | undefined>(undefined);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    setDraft(slices.outbounds);
  }
  if (!slices && draft !== null) {
    setPrevSlices(undefined);
    setDraft(null);
  }

  // ---- 局部 UI 状态 ----
  const [sheetOpen, setSheetOpen] = useState(false);
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

  // 分组成员候选：静态订阅节点（订阅缓存）+ 切片节点出站（enabled）+ 内置 direct；
  // 候选不含其它分组（禁嵌套，与 Rust `validate_group_members` 一致）。
  // 内置 selector（proxy）的默认成员候选：运行时成员 = auto + 订阅节点（切片节点与
  // direct/block 不在内置分组成员中，后端 apply 对悬空 default 保持模板默认并告警）。
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
    setSheetOpen(true);
  };

  const openEdit = (item: CustomOutbound) => {
    setEditing(item);
    setSheetOpen(true);
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
    setSheetOpen(false);
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
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader
        title="出站管理"
        action={
          <Button
            variant="primary"
            className="h-11 shrink-0 px-4"
            isDisabled={!valid || !dirty || saving}
            isPending={saving}
            onPress={() => void handleSave()}
          >
            保存
          </Button>
        }
      />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {isLoading && !slices && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在加载自定义出站配置…</span>
            </Card.Content>
          </Card>
        )}

        {!isLoading && queryError && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>加载失败</Alert.Title>
                  <Alert.Description>{toErrorMessage(queryError)}</Alert.Description>
                </Alert.Content>
              </Alert>
            </Card.Content>
          </Card>
        )}

        {!isLoading && !queryError && !slices && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
              <span className="text-sm text-muted">自定义出站配置不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={invalidate}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
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

        {/* 内置出站：置底只读 */}
      </div>

      {/* 编辑 Sheet 与删除确认（常驻挂载，isOpen / 目标控制显隐） */}
      <OutboundFormSheet
        isOpen={sheetOpen}
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
        onClose={() => setSheetOpen(false)}
        onSave={handleSaveItem}
        onDeleteRequest={handleDeleteRequest}
      />
      <OutboundDeleteConfirm
        isOpen={pendingDelete !== null}
        title="删除自定义出站"
        description={pendingDelete ? `确定删除出站「${pendingDelete.name}」吗？该操作不可撤销。` : ""}
        onClose={() => setPendingDelete(null)}
        onConfirm={handleDeleteConfirm}
      />
    </div>
  );
}
