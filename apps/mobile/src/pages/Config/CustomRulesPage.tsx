import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  buildSaveInput,
  localOverrideGet,
  localOverrideSave,
  ruleSummary,
  toErrorMessage,
  toastError,
  toastSuccess,
  useProxyStatus,
  viewToInput,
} from "@pp/client-core";
import type { CoreLocalOverrideInput, LocalOverrideView, LocalRuleInput, LocalRuleView } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";
import { RuleDeleteConfirm } from "./RuleDeleteConfirm";
import { RuleEditSheet } from "./RuleEditSheet";
import type { RuleSetOption } from "./RuleEditSheet";
import { RuleListSection } from "./RuleListSection";

/**
 * 自定义规则子页（ADR-0003 M5.4 拆分，路由 `/config/rules`）。
 *
 * 承接原规则主页的规则列表 CRUD：启停 / 上移下移 / 添加 / 编辑 / 删除，数据流不变
 * （`persist = buildSaveInput + localOverrideSave` 全量落盘，成功按动作差异化 toast）。
 *
 * 自「场景模板改为规则引用 + 应用激活」起注入条件 = `enabled && 被已应用模板引用`：
 * 未被任何模板引用的规则卡出「未分配模板」warning chip（不会注入启动配置）。
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

  /** 被任意场景模板引用的规则 id 集合（未引用的规则不注入启动配置，见 RuleCard）。 */
  const referencedTemplateRuleIds = useMemo<ReadonlySet<string>>(() => {
    if (!overrideData) return new Set<string>();
    return new Set(overrideData.custom_templates.flatMap((t) => t.rules));
  }, [overrideData]);

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
            referencedRuleIds={referencedTemplateRuleIds}
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
      />
      <RuleDeleteConfirm
        rule={pendingDelete}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => void handleDeleteConfirm()}
      />
    </div>
  );
}
