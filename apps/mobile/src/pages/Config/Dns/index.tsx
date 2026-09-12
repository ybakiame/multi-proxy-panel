import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  CONFIG_SLICES_KEY,
  LOCAL_OVERRIDE_KEY,
  configSlicesGet,
  configSlicesSave,
  localOverrideGet,
  toErrorMessage,
  toastError,
  toastSuccess,
  useCapabilities,
  useProxyStatus,
} from "@pp/client-core";
import type {
  ConfigSlices,
  DnsMode,
  DnsRule,
  DnsServer,
  DnsSlice,
  DnsStrategy,
  LocalOverrideView,
} from "@pp/client-core";
import { BackHeader } from "../../../components/BackHeader";
import { isLocalOverrideView } from "../localOverrideGuards";
import { buildRuleSetOptions } from "../ruleSetOptions";
import { DnsDeleteConfirm } from "./DnsDeleteConfirm";
import { DnsFakeipCard } from "./DnsFakeipCard";
import { DnsModeCard } from "./DnsModeCard";
import { DnsRoutingCard } from "./DnsRoutingCard";
import { DnsRuleFormSheet } from "./DnsRuleFormSheet";
import { DnsRuleListSection } from "./DnsRuleListSection";
import { DnsServerFormSheet } from "./DnsServerFormSheet";
import { DnsServerListSection } from "./DnsServerListSection";
import { dnsRuleSummary, dnsServerTagOptions, isConfigSlices, isDnsSliceValid, validateDnsSlice } from "./dnsUtils";

/**
 * DNS 切片配置子页（ADR-0005 P0-4b，路由 `/config/dns`）。
 *
 * 结构自上而下：BackHeader（右侧保存动作）→ takeover 风险 Alert →
 * DNS 模式（跟随系统 / 接管）→ DNS 服务器列表 → DNS 分流规则列表 →
 * final 服务器 + 全局解析策略。
 * 无切片总开关：`takeover` 模式下本切片正文即注入运行配置。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量 `ConfigSlices`；所有编辑只改内存中的
 * dns 切片草稿（copy-on-write），点击保存才整份 `configSlicesSave` 落盘，成功后
 * invalidate + toast；核心运行中追加「重启代理后生效」。校验（D5）在保存前对整份
 * 草稿执行，失败禁用保存并在字段行内提示。
 */
export default function DnsPage() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const { data: capabilities } = useCapabilities();
  // DNS 切片在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  const isAndroid = capabilities?.is_android ?? false;

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

  // 结构守卫：异构/异常缓存视为未加载，渲染空态而非崩溃。
  const slices = isConfigSlices(rawSlices) ? rawSlices : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });

  // 规则集候选数据源：复用规则页同 key 缓存（LOCAL_OVERRIDE_KEY），不新增请求模式。
  const { data: rawOverride } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const ruleSetOptions = useMemo(() => buildRuleSetOptions(overrideData), [overrideData]);

  // ---- 内存草稿（query 数据变化时渲染期同步，copy-on-write 编辑） ----
  const [draft, setDraft] = useState<DnsSlice | null>(null);
  const [prevSlices, setPrevSlices] = useState<ConfigSlices | undefined>(undefined);
  if (slices && prevSlices !== slices) {
    setPrevSlices(slices);
    setDraft(slices.dns);
  }
  if (!slices && draft !== null) {
    setPrevSlices(undefined);
    setDraft(null);
  }

  // ---- 局部 UI 状态 ----
  const [serverSheetOpen, setServerSheetOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<DnsServer | null>(null);
  const [pendingServerDelete, setPendingServerDelete] = useState<DnsServer | null>(null);
  const [ruleSheetOpen, setRuleSheetOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<DnsRule | null>(null);
  const [pendingRuleDelete, setPendingRuleDelete] = useState<DnsRule | null>(null);
  const [saving, setSaving] = useState(false);

  const errors = draft ? validateDnsSlice(draft) : null;
  const valid = errors !== null && isDnsSliceValid(errors);
  const dirty = draft !== null && slices !== null && JSON.stringify(draft) !== JSON.stringify(slices.dns);
  const serverTagOptions = useMemo(() => (draft ? dnsServerTagOptions(draft.servers) : []), [draft]);
  const serverOtherTags = useMemo(
    () => (draft ? draft.servers.filter((server) => server !== editingServer).map((server) => server.tag.trim()) : []),
    [draft, editingServer],
  );

  const handleSave = async () => {
    if (!slices || !draft || !valid || !dirty || saving) return;
    setSaving(true);
    try {
      await configSlicesSave({ ...slices, dns: draft });
      toastSuccess(coreRunning ? "DNS 配置已保存，重启代理后生效" : "DNS 配置已保存");
      await queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  // ---- 标量字段（模式 / final / 策略） ----
  const handleChangeMode = (mode: DnsMode) => setDraft((current) => (current ? { ...current, mode } : current));
  const handleChangeStrategy = (strategy: DnsStrategy) =>
    setDraft((current) => (current ? { ...current, strategy } : current));
  const handleChangeFinalTag = (final_tag: string) =>
    setDraft((current) => (current ? { ...current, final_tag } : current));

  // ---- DNS 服务器 ----
  const openAddServer = () => {
    setEditingServer(null);
    setServerSheetOpen(true);
  };

  const openEditServer = (server: DnsServer) => {
    setEditingServer(server);
    setServerSheetOpen(true);
  };

  /** 新增/编辑共用：编辑时 tag 变更级联更新引用它的规则与 final。 */
  const handleSaveServer = (server: DnsServer) => {
    setDraft((current) => {
      if (!current) return current;
      if (!editingServer) {
        return { ...current, servers: [...current.servers, server] };
      }
      const oldTag = editingServer.tag;
      const servers = current.servers.map((item) => (item === editingServer ? server : item));
      const final_tag = current.final_tag === oldTag ? server.tag : current.final_tag;
      const rules = current.rules.map((rule) =>
        rule.server_tag === oldTag ? { ...rule, server_tag: server.tag } : rule,
      );
      return { ...current, servers, final_tag, rules };
    });
  };

  const handleServerDeleteRequest = (server: DnsServer) => {
    setServerSheetOpen(false);
    setPendingServerDelete(server);
  };

  const handleServerDeleteConfirm = () => {
    const target = pendingServerDelete;
    setPendingServerDelete(null);
    if (!target || !draft) return;
    const referenced = draft.final_tag === target.tag || draft.rules.some((rule) => rule.server_tag === target.tag);
    if (referenced) {
      toastError(`服务器「${target.tag}」仍被规则或 final 引用，请先调整引用`);
      return;
    }
    setDraft({ ...draft, servers: draft.servers.filter((item) => item !== target) });
  };

  // ---- DNS 分流规则 ----
  const openAddRule = () => {
    setEditingRule(null);
    setRuleSheetOpen(true);
  };

  const openEditRule = (rule: DnsRule) => {
    setEditingRule(rule);
    setRuleSheetOpen(true);
  };

  const handleSaveRule = (rule: DnsRule) => {
    setDraft((current) => {
      if (!current) return current;
      const exists = current.rules.some((item) => item.id === rule.id);
      return exists
        ? { ...current, rules: current.rules.map((item) => (item.id === rule.id ? rule : item)) }
        : { ...current, rules: [...current.rules, rule] };
    });
  };

  const handleToggleRule = (rule: DnsRule, next: boolean) => {
    setDraft((current) =>
      current
        ? { ...current, rules: current.rules.map((item) => (item.id === rule.id ? { ...item, enabled: next } : item)) }
        : current,
    );
  };

  const handleRuleDeleteRequest = (rule: DnsRule) => {
    setRuleSheetOpen(false);
    setPendingRuleDelete(rule);
  };

  const handleRuleDeleteConfirm = () => {
    const target = pendingRuleDelete;
    setPendingRuleDelete(null);
    if (!target) return;
    setDraft((current) =>
      current ? { ...current, rules: current.rules.filter((item) => item.id !== target.id) } : current,
    );
  };

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader
        title="DNS"
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
              <span className="text-sm text-muted">正在加载 DNS 配置…</span>
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
              <span className="text-sm text-muted">DNS 配置不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={invalidate}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        {draft && (
          <>
            {draft.mode === "takeover" && (
              <Alert status="warning">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>DNS 接管风险</Alert.Title>
                  <Alert.Description>
                    接管 DNS 后配置错误可能导致断网，请确保 final 服务器有效；同时需保留一个 tag 为 local 的服务器，供
                    route.default_domain_resolver 与出站域名解析兜底，否则核心可能启动失败
                  </Alert.Description>
                </Alert.Content>
              </Alert>
            )}

            <DnsModeCard mode={draft.mode} isAndroid={isAndroid} onChangeMode={handleChangeMode} />

            {/* FakeIP（W2，ClientConfig）仅在非接管模式可见：接管后由切片正文全权接管 */}
            {draft.mode !== "takeover" && <DnsFakeipCard />}

            <DnsServerListSection servers={draft.servers} onEdit={openEditServer} onAdd={openAddServer} />

            <DnsRuleListSection
              rules={draft.rules}
              onToggle={handleToggleRule}
              onEdit={openEditRule}
              onAdd={openAddRule}
            />

            <DnsRoutingCard
              finalTag={draft.final_tag}
              strategy={draft.strategy}
              serverTagOptions={serverTagOptions}
              finalTagError={errors?.finalTag ?? null}
              onChangeFinalTag={handleChangeFinalTag}
              onChangeStrategy={handleChangeStrategy}
            />
          </>
        )}
      </div>

      {/* 编辑 Sheet 与删除确认（常驻挂载，isOpen / 目标控制显隐） */}
      <DnsServerFormSheet
        isOpen={serverSheetOpen}
        editing={editingServer}
        otherTags={serverOtherTags}
        onClose={() => setServerSheetOpen(false)}
        onSave={handleSaveServer}
        onDeleteRequest={handleServerDeleteRequest}
      />
      <DnsRuleFormSheet
        isOpen={ruleSheetOpen}
        editing={editingRule}
        serverOptions={serverTagOptions}
        ruleSetOptions={ruleSetOptions}
        onClose={() => setRuleSheetOpen(false)}
        onSave={handleSaveRule}
        onDeleteRequest={handleRuleDeleteRequest}
      />
      <DnsDeleteConfirm
        isOpen={pendingServerDelete !== null}
        title="删除 DNS 服务器"
        description={
          pendingServerDelete
            ? `确定删除服务器「${pendingServerDelete.tag}」吗？仍被规则或 final 引用的服务器无法删除。`
            : ""
        }
        onClose={() => setPendingServerDelete(null)}
        onConfirm={handleServerDeleteConfirm}
      />
      <DnsDeleteConfirm
        isOpen={pendingRuleDelete !== null}
        title="删除 DNS 规则"
        description={
          pendingRuleDelete ? `确定删除规则「${dnsRuleSummary(pendingRuleDelete)}」吗？该操作不可撤销。` : ""
        }
        onClose={() => setPendingRuleDelete(null)}
        onConfirm={handleRuleDeleteConfirm}
      />
    </div>
  );
}
