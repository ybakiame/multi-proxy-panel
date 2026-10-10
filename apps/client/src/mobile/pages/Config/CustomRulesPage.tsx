import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { InlineAlert } from "../../components/InlineAlert";
import { Button, Card, Spinner } from "../../components/ui";
import {
  buildSaveInput,
  LOCAL_OVERRIDE_KEY,
  localOverrideGet,
  localOverrideSave,
  markRestartRequired,
  ruleSummary,
  toastError,
  toastSuccess,
  toErrorMessage,
  useProxyStatus,
  useRuleOutboundOptions,
  viewToInput,
} from "@pp/client-core";
import type { CoreLocalOverrideInput, LocalOverrideView, LocalRuleInput, LocalRuleView } from "@pp/client-core";
import { BASELINE_VIEW_KEY, baselineViewGet } from "@pp/client-core";
import type { BaselineView } from "@pp/client-core";
import { SubPageShell } from "../../components/SubPageShell";
import { isLocalOverrideView } from "@pp/client-core";
import { RuleDeleteConfirm } from "./RuleDeleteConfirm";
import { RuleEditSheet } from "./RuleEditSheet";
import type { RuleSetOption } from "./RuleEditSheet";
import { buildRuleSetOptions } from "@pp/client-core";
import { RuleListSection } from "./RuleListSection";

/**
 * 规则管理子页（ADR-0003 M5.4 拆分，路由 `/config/route/rules`）。
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
  // 指定出站候选（订阅节点 + 模板分组 + 切片出站并集）：数据装配已下沉
  // client-core（ADR-0011 D1），桌面 Rules 页同源消费。
  const { options: outboundOptions, subscriptionCacheAvailable } = useRuleOutboundOptions();
  // 内置基线视图（还原模板数据源：缺失的内置规则按它补回）。
  const { data: baseline } = useQuery<BaselineView>({
    queryKey: BASELINE_VIEW_KEY,
    queryFn: baselineViewGet,
    staleTime: Infinity,
    retry: false,
  });
  // 结构守卫（见 localOverrideGuards.ts）：异构/异常缓存视为未加载，渲染空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const currentCore = overrideData ? overrideData.singbox : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  // ---- 局部 UI 状态 ----
  const [editOpen, setEditOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<LocalRuleView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<LocalRuleView | null>(null);

  /**
   * 规则集选择器候选：可引用集合 = 启用中的内置规则集引用 + 全部自定义规则集。
   * 与 DNS 规则表单同源（`buildRuleSetOptions`），规则集是纯资源无启停概念，
   * 注入与否由引用它的规则决定。
   */
  const ruleSetOptions = useMemo<RuleSetOption[]>(() => buildRuleSetOptions(overrideData), [overrideData]);

  const toastRuleSaved = (base: string) => {
    markRestartRequired("rules", coreRunning);
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
  /** 一键还原内置规则：按基线模板补回缺失的内置规则（置顶），已存在的不动。 */
  const handleRestoreBuiltinRules = async () => {
    if (!currentCore || !baseline) return;
    const existing = new Set(currentCore.rules.map((rule) => rule.id));
    const missing = baseline.route_rules.filter((tpl) => !existing.has(tpl.id));
    if (missing.length === 0) {
      toastSuccess("内置规则已完整");
      return;
    }
    const now = Math.floor(Date.now() / 1000);
    const minSort = Math.min(0, ...currentCore.rules.map((rule) => rule.sort_order));
    const restored: LocalRuleInput[] = missing.map((tpl, i) => ({
      id: tpl.id,
      name: tpl.name,
      enabled: true,
      match_type: "rule_set",
      target: tpl.rule_set_tags.join(","),
      action: tpl.outbound,
      no_resolve: false,
      invert: false,
      note: "",
      created_at: now,
      sort_order: minSort - missing.length + i,
      builtin: true,
    }));
    const input = viewToInput(currentCore);
    if (await persist({ ...input, rules: [...restored, ...input.rules] })) {
      toastSuccess("已还原内置规则（置顶）");
    }
  };

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
    <SubPageShell title="规则管理">
      {isLoading && !overrideData && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-zinc-500 dark:text-zinc-400">正在加载规则…</span>
          </Card.Content>
        </Card>
      )}

      {!isLoading && queryError && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
            <InlineAlert kind="danger" title="加载失败">
              {toErrorMessage(queryError)}
            </InlineAlert>
          </Card.Content>
        </Card>
      )}

      {!isLoading && !queryError && !overrideData && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
            <span className="text-sm text-zinc-500 dark:text-zinc-400">规则数据不可用</span>
            <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => void invalidate()}>
              重新加载
            </Button>
          </Card.Content>
        </Card>
      )}

      {overrideData &&
        currentCore &&
        baseline &&
        baseline.route_rules.some((tpl) => !currentCore.rules.some((rule) => rule.id === tpl.id)) && (
          <Button variant="secondary" className="min-h-11 w-full" onPress={() => void handleRestoreBuiltinRules()}>
            还原内置规则（置顶）
          </Button>
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

      {/* 内置基线路由规则：置底只读，用户规则先于基线生效 */}

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
    </SubPageShell>
  );
}
