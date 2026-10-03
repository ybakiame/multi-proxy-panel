import { useState } from "react";
import { Alert, AlertDialog, Button } from "@heroui/react";
import { PlusIcon } from "@heroicons/react/24/outline";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { localOverrideGet, localOverrideSave, toErrorMessage, useProxyStatus } from "@pp/client-core";
import { LOCAL_OVERRIDE_KEY } from "@pp/client-core";
import { BASELINE_VIEW_KEY, baselineViewGet } from "@pp/client-core";
import type { BaselineView } from "@pp/client-core";
import type { CoreLocalOverrideInput, LocalOverrideView, LocalRuleInput, LocalRuleView } from "@pp/client-core";
import { toastError, toastSuccess } from "@pp/client-core";
import { markRestartRequired } from "@pp/client-core";
import { RuleCard } from "./RuleCard";
import { RuleEditModal } from "./RuleEditModal";
import { RuleSetSection } from "./RuleSetSection";
import { buildSaveInput, ruleSummary, viewToInput } from "./types";

export default function Rules() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 本地规则 / 规则集在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;

  const {
    data: overrideData,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  const [editOpen, setEditOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<LocalRuleView | null>(null);
  const [deleteRule, setDeleteRule] = useState<LocalRuleView | null>(null);

  // 内置基线视图（还原模板数据源：缺失的内置规则按它补回；内置规则为普通规则，
  // 可完整编辑/删除，还原是显式用户动作——对齐移动端物化模型语义）。
  const { data: baseline } = useQuery<BaselineView>({
    queryKey: BASELINE_VIEW_KEY,
    queryFn: baselineViewGet,
    staleTime: Infinity,
    retry: false,
  });

  // 单核心（sing-box）：直接消费 singbox 桶。
  const currentCore = overrideData ? overrideData.singbox : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  const persist = async (value: CoreLocalOverrideInput, successMessage: string) => {
    if (!overrideData) return;
    const input = buildSaveInput(overrideData, value);
    try {
      await localOverrideSave(input);
      toastRuleSaved(successMessage);
      markRestartRequired("rules", coreRunning);
      invalidate();
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
    }
  };

  const handleToggleRule = async (id: string) => {
    if (!overrideData || !currentCore) return;
    const target = currentCore.rules.find((r) => r.id === id);
    const nextRules = currentCore.rules.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r));
    const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
    await persist(next, target?.enabled ? "规则已停用" : "规则已启用");
  };

  const handleMoveUp = async (index: number) => {
    if (!overrideData || !currentCore || index <= 0) return;
    const rules = [...currentCore.rules];
    const tmp = rules[index];
    rules[index] = rules[index - 1];
    rules[index - 1] = tmp;
    const next: CoreLocalOverrideInput = {
      ...viewToInput(currentCore),
      rules: rules.map((r, i) => ({ ...r, sort_order: i })),
    };
    await persist(next, "规则顺序已调整");
  };

  const handleMoveDown = async (index: number) => {
    if (!overrideData || !currentCore || index >= currentCore.rules.length - 1) return;
    const rules = [...currentCore.rules];
    const tmp = rules[index];
    rules[index] = rules[index + 1];
    rules[index + 1] = tmp;
    const next: CoreLocalOverrideInput = {
      ...viewToInput(currentCore),
      rules: rules.map((r, i) => ({ ...r, sort_order: i })),
    };
    await persist(next, "规则顺序已调整");
  };

  const handleDelete = async () => {
    if (!overrideData || !currentCore || !deleteRule) return;
    const nextRules = currentCore.rules.filter((r) => r.id !== deleteRule.id);
    const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
    setDeleteRule(null);
    await persist(next, "规则已删除");
  };

  const handleSaveRule = async (rule: LocalRuleInput) => {
    if (!overrideData || !currentCore) return;
    const exists = currentCore.rules.find((r) => r.id === rule.id);
    let nextRules: LocalRuleInput[];
    if (exists) {
      nextRules = currentCore.rules.map((r) =>
        r.id === rule.id ? rule : viewToInput(currentCore).rules.find((rr) => rr.id === r.id)!,
      );
    } else {
      nextRules = [...viewToInput(currentCore).rules, { ...rule, sort_order: currentCore.rules.length }];
    }
    const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
    await persist(next, exists ? "规则已保存" : "规则已添加");
  };

  const error = queryError ? toErrorMessage(queryError) : null;

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
    await persist({ ...input, rules: [...restored, ...input.rules] }, "已还原内置规则（置顶）");
  };

  const hasMissingBuiltin =
    !!currentCore && !!baseline && baseline.route_rules.some((tpl) => !currentCore.rules.some((r) => r.id === tpl.id));

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">规则</h1>
        <p className="text-sm text-muted">本地规则卡片与规则集管理</p>
      </div>

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>加载失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {isLoading && !overrideData && (
        <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
          <span className="text-sm text-muted">正在加载规则配置…</span>
        </div>
      )}

      {overrideData && currentCore && (
        <>
          {/* 规则卡片 */}
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">规则列表</span>
              <div className="flex items-center gap-2">
                {hasMissingBuiltin && (
                  <Button size="sm" variant="secondary" onPress={() => void handleRestoreBuiltinRules()}>
                    还原内置规则（置顶）
                  </Button>
                )}
                <Button
                  size="sm"
                  variant="primary"
                  onPress={() => {
                    setEditingRule(null);
                    setEditOpen(true);
                  }}
                >
                  <PlusIcon className="size-4" />
                  新增规则
                </Button>
              </div>
            </div>
            {currentCore.rules.length === 0 ? (
              <div className="rounded-lg border border-border/60 bg-surface p-6 text-center text-sm text-muted">
                暂无规则，点击「新增规则」创建
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                {currentCore.rules.map((rule, idx) => (
                  <RuleCard
                    key={rule.id}
                    rule={rule}
                    index={idx}
                    total={currentCore.rules.length}
                    onToggle={(id) => void handleToggleRule(id)}
                    onMoveUp={(i) => void handleMoveUp(i)}
                    onMoveDown={(i) => void handleMoveDown(i)}
                    onEdit={(r) => {
                      setEditingRule(r);
                      setEditOpen(true);
                    }}
                    onDelete={(r) => setDeleteRule(r)}
                  />
                ))}
              </div>
            )}
          </div>

          {/* 规则集管理（社区 / 自定义） */}
          <RuleSetSection overrideData={overrideData} coreRunning={coreRunning} onChanged={invalidate} />
        </>
      )}

      <RuleEditModal
        isOpen={editOpen}
        onClose={() => setEditOpen(false)}
        initial={editingRule}
        onSave={(r) => void handleSaveRule(r)}
        ruleSetOptions={(overrideData?.custom_rule_sets ?? []).map((rs) => ({ value: rs.tag, label: rs.tag }))}
      />

      <AlertDialog.Backdrop
        isOpen={deleteRule !== null}
        onOpenChange={(open) => {
          if (!open) setDeleteRule(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除规则</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">
                确定删除规则「{deleteRule ? ruleSummary(deleteRule) : ""}」吗？该操作不可撤销。
              </p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setDeleteRule(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={() => void handleDelete()}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
