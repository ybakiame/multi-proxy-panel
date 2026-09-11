import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  CONFIG_SLICES_KEY,
  LOCAL_OVERRIDE_KEY,
  PROXIES_KEY,
  buildSaveInput,
  configSlicesGet,
  localOverrideGet,
  localOverrideSave,
  outboundTag,
  proxiesList,
  ruleSummary,
  subscriptionNodeTags,
  subscriptionNodeTagsKey,
  toErrorMessage,
  toastError,
  toastSuccess,
  useClientConfig,
  useProxyStatus,
  viewToInput,
} from "@pp/client-core";
import type {
  ConfigSlices,
  CoreLocalOverrideInput,
  LocalOverrideView,
  LocalRuleInput,
  LocalRuleView,
  NodeTagView,
  ProxyList,
} from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";
import { RuleDeleteConfirm } from "./RuleDeleteConfirm";
import { RuleEditSheet } from "./RuleEditSheet";
import type { OutboundOption, RuleSetOption } from "./RuleEditSheet";
import { RuleListSection } from "./RuleListSection";

/**
 * 自定义规则子页（ADR-0003 M5.4 拆分，路由 `/config/rules`）。
 *
 * 承接原规则主页的规则列表 CRUD：启停 / 上移下移 / 添加 / 编辑 / 删除，数据流不变
 * （`persist = buildSaveInput + localOverrideSave` 全量落盘，成功按动作差异化 toast）。
 *
 * `rule_set` 匹配目标为规则集选择器：选项 = **全部**自定义规则集（tag）——规则集
 * 是纯资源无启停概念；编辑已有规则时原 target 不在候选则追加「原值保留」项。
 */
export default function CustomRulesPage() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 规则在核心启动时注入运行配置，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  const {
    data: rawOverride,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });
  // 指定出站候选数据源（复用各页同 key 缓存，不在 Sheet 内新起重型查询）：
  // 切片出站读 config_slices；订阅节点读生效订阅的本地缓存（静态源，不依赖核心运行）；
  // 模板分组读运行中核心的 proxies_list（PROXIES_KEY，与首页 / 代理页共享缓存，未运行时不发起）。
  const { data: slices } = useQuery<ConfigSlices>({
    queryKey: CONFIG_SLICES_KEY,
    queryFn: configSlicesGet,
  });
  const { data: config } = useClientConfig();
  const activeSubscriptionId = config?.active_subscription_id ?? null;
  const { data: subscriptionNodes } = useQuery<NodeTagView[]>({
    queryKey: subscriptionNodeTagsKey(activeSubscriptionId ?? ""),
    queryFn: () => subscriptionNodeTags(activeSubscriptionId ?? ""),
    enabled: !!activeSubscriptionId,
    retry: false,
  });
  const { data: proxyList } = useQuery<ProxyList>({
    queryKey: PROXIES_KEY,
    queryFn: proxiesList,
    enabled: coreRunning,
    retry: false,
  });
  // 生效订阅存在但缓存为空（从未同步 / 缓存丢失）：给「先同步订阅」引导文案。
  const subscriptionCacheAvailable = !!activeSubscriptionId && (subscriptionNodes?.length ?? 0) > 0;

  // 结构守卫（见 localOverrideGuards.ts）：异构/异常缓存视为未加载，渲染空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const currentCore = overrideData ? overrideData.singbox : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  // ---- 局部 UI 状态 ----
  const [editOpen, setEditOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<LocalRuleView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<LocalRuleView | null>(null);

  /**
   * 规则集选择器候选：全部自定义规则集（社区 remote / 自定义 manual）。自
   * 「规则集移除 enabled」起规则集是纯资源无启停概念，注入与否由引用它的规则决定。
   */
  const ruleSetOptions = useMemo<RuleSetOption[]>(() => {
    if (!overrideData) return [];
    return asArray(overrideData.custom_rule_sets).map((rs) => ({
      value: rs.tag,
      label: rs.tag,
      hint: rs.name.trim() || undefined,
    }));
  }, [overrideData]);

  /**
   * 指定出站候选 tag 并集（ADR-0005 §3.1）：静态订阅节点 + 运行中模板分组 + 切片出站
   * （enabled），按 tag 去重。
   *
   * - 静态订阅节点：`subscription_node_tags`（生效订阅的本地缓存），不依赖核心运行；
   * - 模板分组：`proxiesList.groups`（`PROXIES_KEY`，经 Clash API 读取运行中核心），
   *   核心未运行时无数据；订阅节点不在其中（已由静态源覆盖）；
   * - 切片出站：tag 由名称生成（`outboundTag`），仅列启用项（父切片总开关由注入层判定）。
   */
  const outboundOptions = useMemo<OutboundOption[]>(() => {
    const options: OutboundOption[] = [];
    const seen = new Set<string>();
    const push = (value: string, label: string, hint?: string) => {
      const tag = value.trim();
      if (tag === "" || seen.has(tag)) return;
      seen.add(tag);
      options.push({ value: tag, label, hint });
    };
    for (const node of subscriptionNodes ?? []) push(node.tag, node.name.trim() || node.tag, "订阅节点");
    for (const group of proxyList?.groups ?? []) push(group.name, group.name, "模板分组");
    for (const item of slices?.outbounds.items ?? []) {
      if (!item.enabled) continue;
      const tag = outboundTag(item.name);
      push(tag, item.name, tag);
    }
    return options;
  }, [subscriptionNodes, proxyList, slices]);

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** 规则写操作统一落盘：全量 patch，失败 toast 并保留现场；返回是否成功。 */
  const persist = async (value: CoreLocalOverrideInput): Promise<boolean> => {
    if (!overrideData) return false;
    try {
      await localOverrideSave(buildSaveInput(overrideData, value));
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
      return false;
    }
  };

  const handleToggleRule = async (rule: LocalRuleView, next: boolean) => {
    if (!currentCore) return;
    const nextRules = currentCore.rules.map((r) => (r.id === rule.id ? { ...r, enabled: next } : r));
    if (await persist({ ...viewToInput(currentCore), rules: nextRules })) {
      toastRuleSaved(next ? `已启用规则「${ruleSummary(rule)}」` : `已停用规则「${ruleSummary(rule)}」`);
    }
  };

  /** 上移 / 下移（dir = -1 | 1），重排后按新位置回写 sort_order。 */
  const handleMoveRule = async (index: number, dir: -1 | 1) => {
    if (!currentCore) return;
    const target = index + dir;
    if (target < 0 || target >= currentCore.rules.length) return;
    const rules = [...currentCore.rules];
    [rules[index], rules[target]] = [rules[target], rules[index]];
    const next: CoreLocalOverrideInput = {
      ...viewToInput(currentCore),
      rules: rules.map((r, i) => ({ ...r, sort_order: i })),
    };
    if (await persist(next)) {
      toastRuleSaved("规则顺序已更新");
    }
  };

  /** 新增/编辑共用：已存在替换，否则追加并赋予末尾 sort_order。 */
  const handleSaveRule = async (rule: LocalRuleInput): Promise<boolean> => {
    if (!currentCore) return false;
    const exists = currentCore.rules.some((r) => r.id === rule.id);
    const nextRules = exists
      ? currentCore.rules.map((r) => (r.id === rule.id ? rule : r))
      : [...currentCore.rules, { ...rule, sort_order: currentCore.rules.length }];
    const ok = await persist({ ...viewToInput(currentCore), rules: nextRules });
    if (ok) {
      toastRuleSaved(exists ? "规则已更新" : "规则已添加");
    }
    return ok;
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target || !currentCore) return;
    setPendingDelete(null);
    const nextRules = currentCore.rules.filter((r) => r.id !== target.id);
    if (await persist({ ...viewToInput(currentCore), rules: nextRules })) {
      toastRuleSaved("规则已删除");
    }
  };

  const openAdd = () => {
    setEditingRule(null);
    setEditOpen(true);
  };

  const openEdit = (rule: LocalRuleView) => {
    setEditingRule(rule);
    setEditOpen(true);
  };

  const handleDeleteRequest = (rule: LocalRuleView) => {
    // 编辑 Sheet 内的「删除规则」入口：先收起 Sheet，再弹 AlertDialog 确认。
    setEditOpen(false);
    setPendingDelete(rule);
  };

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="自定义规则" />
      <div
        className="flex min-h-full flex-1 flex-col gap-3 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {isLoading && !overrideData && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在加载规则…</span>
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

        {!isLoading && !queryError && !overrideData && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
              <span className="text-sm text-muted">规则数据不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => void invalidate()}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        {overrideData && currentCore && (
          <RuleListSection
            rules={currentCore.rules}
            onToggle={(rule, next) => void handleToggleRule(rule, next)}
            onMove={(index, dir) => void handleMoveRule(index, dir)}
            onEdit={(rule) => openEdit(rule)}
            onAdd={openAdd}
          />
        )}
      </div>

      {/* 编辑 Sheet 与删除确认（常驻挂载，isOpen / rule 控制显隐） */}
      <RuleEditSheet
        isOpen={editOpen}
        editing={editingRule}
        onClose={() => setEditOpen(false)}
        onSave={(rule) => handleSaveRule(rule)}
        onDeleteRequest={(rule) => handleDeleteRequest(rule)}
        ruleSetOptions={ruleSetOptions}
        outboundOptions={outboundOptions}
        subscriptionCacheAvailable={subscriptionCacheAvailable}
      />
      <RuleDeleteConfirm
        rule={pendingDelete}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => void handleDeleteConfirm()}
      />
    </div>
  );
}
