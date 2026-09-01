import { useCallback, useEffect, useMemo, useState } from "react";
import { Alert, AlertDialog, Button, Card, Switch } from "@heroui/react";
import { PlusIcon } from "@heroicons/react/24/outline";
import {
  localOverrideApplyTemplate,
  localOverrideGet,
  localOverrideRevertTemplate,
  localOverrideRulesets,
  localOverrideSave,
  localOverrideToggleRuleset,
  localOverrideUpdateRulesetsNow,
  toErrorMessage,
} from "../../api";
import type {
  CoreLocalOverrideInput,
  LocalOverrideView,
  LocalRuleInput,
  LocalRuleView,
  RuleSetStatusView,
} from "../../api";
import { useCapabilities } from "../../hooks/useCapabilities";
import { MobileBackHeader } from "../../layout/MobileBackHeader";
import { toastError, toastSuccess } from "../../toast";
import { RuleCard } from "./RuleCard";
import { RuleEditModal } from "./RuleEditModal";
import { RuleSetTable } from "./RuleSetTable";
import { TEMPLATE_DEFS } from "./types";
import { buildSaveInput, ruleSummary, viewToInput } from "./types";
import { TemplateCard } from "./TemplateCard";

export default function Rules() {
  const { data: capabilities } = useCapabilities();
  const isAndroid = capabilities?.is_android ?? false;

  const [data, setData] = useState<LocalOverrideView | null>(null);
  const [ruleSets, setRuleSets] = useState<RuleSetStatusView[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const [editOpen, setEditOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<LocalRuleView | null>(null);
  const [deleteRule, setDeleteRule] = useState<LocalRuleView | null>(null);
  const [busy, setBusy] = useState(false);

  const coreKey: "singbox" | "mihomo" = isAndroid
    ? "singbox"
    : (data?.singbox.rules.length ?? 0) >= (data?.mihomo.rules.length ?? 0)
      ? "singbox"
      : "mihomo";

  const currentCore = data ? data[coreKey] : null;

  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const [ovr, rs] = await Promise.all([localOverrideGet(), localOverrideRulesets()]);
      setData(ovr);
      setRuleSets(rs);
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const persist = useCallback(
    async (patchCore: { key: "singbox" | "mihomo"; value: CoreLocalOverrideInput }) => {
      if (!data) return;
      const input = buildSaveInput(data, patchCore);
      try {
        await localOverrideSave(input);
        toastSuccess("已保存");
        setData((prev) => {
          if (!prev) return prev;
          return {
            ...prev,
            [patchCore.key]: { ...patchCore.value, rules: patchCore.value.rules, rule_sets: patchCore.value.rule_sets },
          };
        });
      } catch (err) {
        toastError(toErrorMessage(err));
        await loadData();
      }
    },
    [data, loadData],
  );

  const handleToggleEnabled = useCallback(async () => {
    if (!data || !currentCore) return;
    const next: CoreLocalOverrideInput = {
      ...viewToInput(currentCore),
      enabled: !currentCore.enabled,
    };
    await persist({ key: coreKey, value: next });
  }, [data, currentCore, coreKey, persist]);

  const handleToggleRule = useCallback(
    async (id: string) => {
      if (!data || !currentCore) return;
      const nextRules = currentCore.rules.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r));
      const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
      await persist({ key: coreKey, value: next });
    },
    [data, currentCore, coreKey, persist],
  );

  const handleMoveUp = useCallback(
    async (index: number) => {
      if (!data || !currentCore || index <= 0) return;
      const rules = [...currentCore.rules];
      const tmp = rules[index];
      rules[index] = rules[index - 1];
      rules[index - 1] = tmp;
      const next: CoreLocalOverrideInput = {
        ...viewToInput(currentCore),
        rules: rules.map((r, i) => ({ ...r, sort_order: i })),
      };
      await persist({ key: coreKey, value: next });
    },
    [data, currentCore, coreKey, persist],
  );

  const handleMoveDown = useCallback(
    async (index: number) => {
      if (!data || !currentCore || index >= currentCore.rules.length - 1) return;
      const rules = [...currentCore.rules];
      const tmp = rules[index];
      rules[index] = rules[index + 1];
      rules[index + 1] = tmp;
      const next: CoreLocalOverrideInput = {
        ...viewToInput(currentCore),
        rules: rules.map((r, i) => ({ ...r, sort_order: i })),
      };
      await persist({ key: coreKey, value: next });
    },
    [data, currentCore, coreKey, persist],
  );

  const handleDelete = useCallback(async () => {
    if (!data || !currentCore || !deleteRule) return;
    const nextRules = currentCore.rules.filter((r) => r.id !== deleteRule.id);
    const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
    setDeleteRule(null);
    await persist({ key: coreKey, value: next });
  }, [data, currentCore, deleteRule, coreKey, persist]);

  const handleSaveRule = useCallback(
    async (rule: LocalRuleInput) => {
      if (!data || !currentCore) return;
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
      await persist({ key: coreKey, value: next });
    },
    [data, currentCore, coreKey, persist],
  );

  const handleApplyTemplate = useCallback(
    async (templateId: string) => {
      setBusy(true);
      try {
        await localOverrideApplyTemplate(templateId);
        toastSuccess("模板已应用");
        await loadData();
      } catch (err) {
        toastError(toErrorMessage(err));
      } finally {
        setBusy(false);
      }
    },
    [loadData],
  );

  const handleRevertTemplate = useCallback(
    async (templateId: string) => {
      setBusy(true);
      try {
        await localOverrideRevertTemplate(templateId);
        toastSuccess("模板已撤销");
        await loadData();
      } catch (err) {
        toastError(toErrorMessage(err));
      } finally {
        setBusy(false);
      }
    },
    [loadData],
  );

  const handleToggleRuleset = useCallback(
    async (communityId: string, subscribed: boolean) => {
      try {
        await localOverrideToggleRuleset(communityId, subscribed);
        toastSuccess(subscribed ? "已订阅" : "已取消订阅");
        await loadData();
      } catch (err) {
        toastError(toErrorMessage(err));
      }
    },
    [loadData],
  );

  const handleUpdateRulesetsNow = useCallback(async () => {
    setBusy(true);
    try {
      const updated = await localOverrideUpdateRulesetsNow();
      toastSuccess(`已更新 ${updated} 个规则集`);
      await loadData();
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  }, [loadData]);

  const appliedTemplateIds = useMemo(() => new Set(data?.applied_templates.map((t) => t.template_id) ?? []), [data]);

  return (
    <div className="flex flex-col gap-6">
      <MobileBackHeader title="规则" />
      <div>
        <h1 className="text-xl font-semibold">规则</h1>
        <p className="text-sm text-muted">本地规则卡片、场景模板与规则集订阅管理</p>
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

      {loading && !data && (
        <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
          <span className="text-sm text-muted">正在加载规则配置…</span>
        </div>
      )}

      {data && currentCore && (
        <>
          {/* 总开关 */}
          <Card>
            <Card.Header>
              <Card.Title>本地规则总开关</Card.Title>
              <Card.Description>控制当前核心的本地 Override 是否生效</Card.Description>
            </Card.Header>
            <Card.Content>
              <Switch isSelected={currentCore.enabled} onChange={() => void handleToggleEnabled()}>
                <Switch.Content>
                  <Switch.Control>
                    <Switch.Thumb />
                  </Switch.Control>
                  启用本地规则（{coreKey === "singbox" ? "sing-box" : "mihomo"}）
                </Switch.Content>
              </Switch>
            </Card.Content>
          </Card>

          {/* 场景模板 */}
          <div className="flex flex-col gap-3">
            <span className="text-sm font-medium">场景模板</span>
            <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
              {TEMPLATE_DEFS.map((t) => (
                <TemplateCard
                  key={t.id}
                  template={t}
                  applied={appliedTemplateIds.has(t.id)}
                  onApply={handleApplyTemplate}
                  onRevert={handleRevertTemplate}
                  busy={busy}
                />
              ))}
            </div>
          </div>

          {/* 规则卡片 */}
          <div className="flex flex-col gap-3">
            <div className="flex items-center justify-between">
              <span className="text-sm font-medium">规则列表</span>
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

          {/* 规则集订阅 */}
          <RuleSetTable
            ruleSets={ruleSets}
            onToggle={(id, sub) => void handleToggleRuleset(id, sub)}
            onUpdateNow={() => void handleUpdateRulesetsNow()}
            busy={busy}
          />
        </>
      )}

      <RuleEditModal
        isOpen={editOpen}
        onClose={() => setEditOpen(false)}
        initial={editingRule}
        isAndroid={isAndroid}
        onSave={(r) => void handleSaveRule(r)}
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
              <Button slot="close" variant="danger" isPending={busy} onPress={() => void handleDelete()}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
