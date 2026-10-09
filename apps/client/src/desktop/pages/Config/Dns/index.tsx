import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, AlertDialog, Button } from "@heroui/react";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import {
  BUILTIN_DNS_SLICE_KEY,
  builtinDnsSliceGet,
  CONFIG_SLICES_KEY,
  configSlicesGet,
  configSlicesSave,
  defaultConfigSlices,
  LOCAL_OVERRIDE_KEY,
  localOverrideGet,
  markRestartRequired,
  toastError,
  toastSuccess,
  toErrorMessage,
  useProxyStatus,
} from "@pp/client-core";
import type {
  ConfigSlices,
  DnsRule,
  DnsServer,
  DnsServerPreset,
  DnsSlice,
  DnsStrategy,
  LocalOverrideView,
} from "@pp/client-core";
import {
  buildRuleSetOptions,
  dnsRuleSummary,
  dnsServerTagOptions,
  dnsSliceEquals,
  isConfigSlices,
  isDnsSliceValid,
  isLocalOverrideView,
  useDnsServers,
  validateDnsSlice,
} from "@pp/client-core";
import { DnsFakeipCard } from "./DnsFakeipCard";
import { DnsPresetModal } from "./DnsPresetModal";
import { DnsRolePickerModal } from "./DnsRolePickerModal";
import { DnsRoutingCard } from "./DnsRoutingCard";
import { DnsRuleFormModal } from "./DnsRuleFormModal";
import { DnsRuleListSection } from "./DnsRuleListSection";
import { DnsServerFormModal } from "./DnsServerFormModal";
import { DnsServerListSection } from "./DnsServerListSection";

/**
 * DNS 切片配置页（桌面端，路由 `/config/dns`；语义对齐移动端 `Config/Dns/index.tsx`）。
 *
 * 结构自上而下：页头（返回 + 保存）→ 「恢复默认配置」（接管草稿时出现）→ FakeIP
 * 开关（仅跟随系统时可见）→ DNS 服务器列表 → DNS 分流规则列表 → final 服务器 +
 * 全局解析策略 + reverse_mapping。
 *
 * 内置默认映射：`builtinDnsSliceGet` 返回与运行核心同形的内置 DNS（切片 schema），
 * `follow_system` 模式下编辑器直接以其初始化草稿；保存时内容与内置默认一致则保持
 * `follow_system`，有任何改动则落为 `takeover` 生效（编辑即接管）。
 *
 * 数据流：`useQuery(CONFIG_SLICES_KEY)` 取全量切片；编辑只改内存草稿
 * （copy-on-write），保存才整份 `configSlicesSave` 落盘，成功后 invalidate + toast
 * 并上报重启脏标记；校验在保存前对整份草稿执行，失败禁用保存。
 */
export default function DnsPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // DNS 切片在核心启动时注入，运行中变更不热更新：保存成功上报全局脏标记。
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
  const [presetOpen, setPresetOpen] = useState(false);
  const [serverFormOpen, setServerFormOpen] = useState(false);
  // 服务器库逻辑（启用/弃用、预置物化、批量探测）复用 client-core hook。
  const { probes, probing, handleToggleServer, handlePickPreset, handleProbeServers } = useDnsServers(setDraft);
  const [editingServer, setEditingServer] = useState<DnsServer | null>(null);
  // 内置角色服务器（local / proxy）更换目录：非 null 时弹出选择弹窗。
  const [rolePick, setRolePick] = useState<DnsServer | null>(null);
  const [pendingServerDelete, setPendingServerDelete] = useState<DnsServer | null>(null);
  const [ruleFormOpen, setRuleFormOpen] = useState(false);
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
      markRestartRequired("dns", coreRunning);
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
  /** 添加入口：先出预置目录弹窗；「自定义添加」再切到编辑表单。 */
  const openAddServer = () => {
    setPresetOpen(true);
  };

  const openCustomServer = () => {
    setPresetOpen(false);
    setEditingServer(null);
    setServerFormOpen(true);
  };

  const openEditServer = (server: DnsServer) => {
    setEditingServer(server);
    setServerFormOpen(true);
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

  /**
   * 内置角色服务器（local / proxy）原位替换：tag 与 enabled（强制重新启用）不变，
   * 名称/地址/类型/端口/出站取预置项——引用该 tag 的分流规则与 final 无需变动。
   */
  const handleRolePresetPick = (target: DnsServer, preset: DnsServerPreset) => {
    setRolePick(null);
    setDraft((current) =>
      current
        ? {
            ...current,
            servers: current.servers.map((server) =>
              server === target
                ? {
                    ...server,
                    name: preset.isp,
                    enabled: true,
                    server: preset.server,
                    server_type: preset.serverType,
                    server_port: preset.serverPort,
                    detour: preset.detour ?? "",
                    domain_resolver: "",
                  }
                : server,
            ),
          }
        : current,
    );
  };

  const handleServerDeleteRequest = (server: DnsServer) => {
    setServerFormOpen(false);
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
    setRuleFormOpen(true);
  };

  const openEditRule = (rule: DnsRule) => {
    setEditingRule(rule);
    setRuleFormOpen(true);
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
    setRuleFormOpen(false);
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
    <div className="flex max-w-2xl flex-col gap-6">
      {/* 页头：返回 + 标题 + 保存 */}
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Button size="sm" variant="ghost" isIconOnly aria-label="返回配置管理" onPress={() => navigate("/config")}>
            <ArrowLeftIcon className="size-4" />
          </Button>
          <div>
            <h1 className="text-xl font-semibold">DNS 管理</h1>
            <p className="text-sm text-muted">DNS 服务器、分流规则与 FakeIP</p>
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
          <span className="text-sm text-muted">正在加载 DNS 配置…</span>
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
          <span className="text-sm text-muted">DNS 配置不可用</span>
          <Button size="sm" variant="secondary" onPress={invalidate}>
            重新加载
          </Button>
        </div>
      )}

      {draft && (
        <>
          {willTakeover && (
            <Button variant="secondary" onPress={handleRestoreBuiltin} isDisabled={builtin === undefined}>
              恢复默认配置
            </Button>
          )}

          {/* FakeIP（ClientConfig）仅在内置默认（跟随系统）生效时可见：接管后由切片正文全权接管 */}
          {!willTakeover && <DnsFakeipCard />}

          <DnsServerListSection
            servers={draft.servers}
            probes={probes}
            probing={probing}
            onToggle={handleToggleServer}
            onProbe={() => void handleProbeServers(draft)}
            onEdit={openEditServer}
            onAdd={openAddServer}
            onRolePick={setRolePick}
          />

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

      {/* 编辑弹窗与删除确认（常驻挂载，isOpen / 目标控制显隐） */}
      <DnsPresetModal
        isOpen={presetOpen}
        existingTags={draft ? draft.servers.map((server) => server.tag) : []}
        onClose={() => setPresetOpen(false)}
        onPick={(preset) => {
          setPresetOpen(false);
          handlePickPreset(preset);
        }}
        onCustom={openCustomServer}
      />
      <DnsRolePickerModal target={rolePick} onClose={() => setRolePick(null)} onPick={handleRolePresetPick} />
      <DnsServerFormModal
        isOpen={serverFormOpen}
        editing={editingServer}
        otherTags={serverOtherTags}
        onClose={() => setServerFormOpen(false)}
        onSave={handleSaveServer}
        onDeleteRequest={handleServerDeleteRequest}
      />
      <DnsRuleFormModal
        isOpen={ruleFormOpen}
        editing={editingRule}
        serverOptions={serverTagOptions}
        ruleSetOptions={ruleSetOptions}
        onClose={() => setRuleFormOpen(false)}
        onSave={handleSaveRule}
        onDeleteRequest={handleRuleDeleteRequest}
      />

      <AlertDialog.Backdrop
        isOpen={pendingServerDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingServerDelete(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除 DNS 服务器</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">
                确定删除服务器「{pendingServerDelete?.tag}」吗？仍被规则或 final 引用的服务器无法删除。
              </p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingServerDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={handleServerDeleteConfirm}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>

      <AlertDialog.Backdrop
        isOpen={pendingRuleDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingRuleDelete(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除 DNS 规则</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">
                确定删除规则「{pendingRuleDelete ? dnsRuleSummary(pendingRuleDelete) : ""}」吗？该操作不可撤销。
              </p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingRuleDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={handleRuleDeleteConfirm}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
