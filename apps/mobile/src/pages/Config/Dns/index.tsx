import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  BUILTIN_DNS_SLICE_KEY,
  CONFIG_SLICES_KEY,
  LOCAL_OVERRIDE_KEY,
  builtinDnsSliceGet,
  configSlicesGet,
  configSlicesSave,
  defaultConfigSlices,
  localOverrideGet,
  toErrorMessage,
  toastError,
  toastSuccess,
  useProxyStatus,
} from "@pp/client-core";
import type { ConfigSlices, DnsRule, DnsServer, DnsSlice, DnsStrategy, LocalOverrideView } from "@pp/client-core";
import { BackHeader } from "../../../components/BackHeader";
import { isLocalOverrideView } from "../localOverrideGuards";
import { buildRuleSetOptions } from "../ruleSetOptions";
import { DnsDeleteConfirm } from "./DnsDeleteConfirm";
import { DnsFakeipCard } from "./DnsFakeipCard";
import { DnsRoutingCard } from "./DnsRoutingCard";
import { DnsRuleFormSheet } from "./DnsRuleFormSheet";
import { DnsRuleListSection } from "./DnsRuleListSection";
import { DnsServerFormSheet } from "./DnsServerFormSheet";
import { DnsServerListSection } from "./DnsServerListSection";
import {
  dnsRuleSummary,
  dnsServerTagOptions,
  dnsSliceEquals,
  isConfigSlices,
  isDnsSliceValid,
  validateDnsSlice,
} from "./dnsUtils";

/**
 * DNS 切片配置子页（ADR-0005 P0-4b，路由 `/config/dns`；2026-09 补记：内置默认映射 +
 * 编辑即接管）。
 *
 * 结构自上而下：BackHeader（右侧保存动作）→ 接管风险 Alert → 配置来源状态卡（内置默认 /
 * 自定义接管，含「恢复内置默认」）→ DNS 服务器列表 → DNS 分流规则列表 →
 * final 服务器 + 全局解析策略 + reverse_mapping。
 *
 * 内置默认映射：`builtinDnsSliceGet` 返回与运行核心完全同形的内置 DNS（切片 schema），
 * `follow_system` 模式下编辑器直接以其初始化草稿——内置配置不再黑盒，无需手动切换接管
 * 模式；保存时内容与内置默认一致则保持 `follow_system`（正文清空回退），有任何改动则
 * 落为 `takeover` 生效。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量 `ConfigSlices` + `useQuery(BUILTIN_DNS_SLICE_KEY)`
 * 取内置默认；所有编辑只改内存中的 dns 切片草稿（copy-on-write），点击保存才整份
 * `configSlicesSave` 落盘，成功后 invalidate + toast；核心运行中追加「重启代理后生效」。
 * 校验（D5）在保存前对整份草稿执行，失败禁用保存并在字段行内提示。
 */
export default function DnsPage() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // DNS 切片在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
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

  // 内置默认 DNS（切片 schema 形态，与运行核心同形）；跟随系统模式的草稿初值来源。
  const { data: builtin } = useQuery<DnsSlice>({
    queryKey: BUILTIN_DNS_SLICE_KEY,
    queryFn: builtinDnsSliceGet,
    refetchOnWindowFocus: false,
  });

  // 当前生效内容：跟随系统 = 内置默认（映射进编辑器），接管 = 切片正文。
  const currentEffective = useMemo<DnsSlice | null>(() => {
    if (!slices) return null;
    return slices.dns.mode === "follow_system" && builtin ? { ...builtin, mode: "follow_system" } : slices.dns;
  }, [slices, builtin]);

  // ---- 内存草稿（query 数据变化时渲染期同步，copy-on-write 编辑） ----
  // 生效内容身份变化时同步草稿；用户已有未保存编辑（草稿偏离旧生效内容）时不覆盖。
  const [draft, setDraft] = useState<DnsSlice | null>(null);
  const [prevEffective, setPrevEffective] = useState<DnsSlice | null>(null);
  if (currentEffective && prevEffective !== currentEffective) {
    setDraft((current) =>
      current === null || dnsSliceEquals(current, prevEffective ?? current) ? currentEffective : current,
    );
    setPrevEffective(currentEffective);
  }
  if (!currentEffective && draft !== null) {
    setPrevEffective(null);
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
  const dirty = draft !== null && currentEffective !== null && !dnsSliceEquals(draft, currentEffective);
  // 编辑即接管：草稿内容与内置默认一致 → 保存后仍跟随系统；有改动 → 落 takeover。
  const contentIsBuiltin =
    draft !== null &&
    builtin !== undefined &&
    dnsSliceEquals({ ...draft, mode: "follow_system" }, { ...builtin, mode: "follow_system" });
  const willTakeover = draft !== null && !contentIsBuiltin;
  const serverTagOptions = useMemo(() => (draft ? dnsServerTagOptions(draft.servers) : []), [draft]);
  const serverOtherTags = useMemo(
    () => (draft ? draft.servers.filter((server) => server !== editingServer).map((server) => server.tag.trim()) : []),
    [draft, editingServer],
  );

  const handleSave = async () => {
    if (!slices || !draft || !valid || !dirty || saving) return;
    setSaving(true);
    try {
      // 编辑即接管：内容与内置默认一致 → 清空正文保持 follow_system；有改动 → takeover 落盘。
      const nextDns: DnsSlice = contentIsBuiltin ? { ...defaultConfigSlices().dns } : { ...draft, mode: "takeover" };
      await configSlicesSave({ ...slices, dns: nextDns });
      toastSuccess(coreRunning ? "DNS 配置已保存，重启代理后生效" : "DNS 配置已保存");
      await queryClient.invalidateQueries({ queryKey: CONFIG_SLICES_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  /** 恢复内置默认：草稿回到内置视图（跟随系统），点保存后生效。 */
  const handleRestoreBuiltin = () => {
    if (!builtin) return;
    setDraft({ ...builtin, mode: "follow_system" });
  };

  // ---- 标量字段（final / 策略 / reverse_mapping） ----
  const handleChangeStrategy = (strategy: DnsStrategy) =>
    setDraft((current) => (current ? { ...current, strategy } : current));
  const handleChangeFinalTag = (final_tag: string) =>
    setDraft((current) => (current ? { ...current, final_tag } : current));
  const handleChangeReverseMapping = (reverse_mapping: boolean) =>
    setDraft((current) => (current ? { ...current, reverse_mapping } : current));

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
        title="DNS 管理"
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
            {willTakeover && (
              <Alert status="warning">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>DNS 接管风险</Alert.Title>
                  <Alert.Description>
                    当前为自定义 DNS（保存后接管生效），配置错误可能导致断网，请确保 final 服务器有效；同时需保留一个
                    tag 为 local 的服务器，供 route.default_domain_resolver 与出站域名解析兜底，否则核心可能启动失败
                  </Alert.Description>
                </Alert.Content>
              </Alert>
            )}

            {/* 配置来源状态卡：内置默认（跟随系统）/ 自定义（接管），接管时可一键恢复内置默认 */}
            <Card>
              <Card.Header>
                <Card.Title>配置来源</Card.Title>
                <Card.Description>
                  {willTakeover
                    ? "自定义 DNS：保存后取代内置默认生效"
                    : "内置默认 DNS：可直接编辑下方配置，保存后自动接管生效"}
                </Card.Description>
              </Card.Header>
              {willTakeover && (
                <Card.Content>
                  <Button
                    variant="secondary"
                    className="min-h-11 w-full"
                    onPress={handleRestoreBuiltin}
                    isDisabled={builtin === undefined}
                  >
                    恢复内置默认
                  </Button>
                </Card.Content>
              )}
            </Card>

            {/* FakeIP（W2，ClientConfig）仅在内置默认（跟随系统）生效时可见：接管后由切片正文全权接管 */}
            {!willTakeover && <DnsFakeipCard />}

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
              reverseMapping={draft.reverse_mapping}
              serverTagOptions={serverTagOptions}
              finalTagError={errors?.finalTag ?? null}
              onChangeFinalTag={handleChangeFinalTag}
              onChangeStrategy={handleChangeStrategy}
              onChangeReverseMapping={handleChangeReverseMapping}
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
