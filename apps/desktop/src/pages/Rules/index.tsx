import { useState } from "react";
import { Alert, AlertDialog, Button, Card, Switch } from "@heroui/react";
import { PlusIcon } from "@heroicons/react/24/outline";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { localOverrideGet, localOverrideSave, toErrorMessage } from "@pp/client-core";
import { LOCAL_OVERRIDE_KEY } from "@pp/client-core";
import type { CoreLocalOverrideInput, LocalOverrideView, LocalRuleInput, LocalRuleView } from "@pp/client-core";
import { toastError, toastSuccess } from "@pp/client-core";
import { RuleCard } from "./RuleCard";
import { RuleEditModal } from "./RuleEditModal";
import { buildSaveInput, ruleSummary, viewToInput } from "./types";

export default function Rules() {
  const queryClient = useQueryClient();

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

  // 单核心（sing-box）：直接消费 singbox 桶。
  const currentCore = overrideData ? overrideData.singbox : null;

  const persist = async (value: CoreLocalOverrideInput) => {
    if (!overrideData) return;
    const input = buildSaveInput(overrideData, value);
    try {
      await localOverrideSave(input);
      toastSuccess("已保存");
      void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });
    } catch (err) {
      toastError(toErrorMessage(err));
      void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });
    }
  };

  const handleToggleEnabled = async () => {
    if (!overrideData || !currentCore) return;
    const next: CoreLocalOverrideInput = {
      ...viewToInput(currentCore),
      enabled: !currentCore.enabled,
    };
    await persist(next);
  };

  const handleToggleRule = async (id: string) => {
    if (!overrideData || !currentCore) return;
    const nextRules = currentCore.rules.map((r) => (r.id === id ? { ...r, enabled: !r.enabled } : r));
    const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
    await persist(next);
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
    await persist(next);
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
    await persist(next);
  };

  const handleDelete = async () => {
    if (!overrideData || !currentCore || !deleteRule) return;
    const nextRules = currentCore.rules.filter((r) => r.id !== deleteRule.id);
    const next: CoreLocalOverrideInput = { ...viewToInput(currentCore), rules: nextRules };
    setDeleteRule(null);
    await persist(next);
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
    await persist(next);
  };

  const error = queryError ? toErrorMessage(queryError) : null;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">规则</h1>
        <p className="text-sm text-muted">本地规则卡片管理（自定义规则集为唯一规则集来源）</p>
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
                  启用本地规则（sing-box）
                </Switch.Content>
              </Switch>
            </Card.Content>
          </Card>

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
        </>
      )}

      <RuleEditModal
        isOpen={editOpen}
        onClose={() => setEditOpen(false)}
        initial={editingRule}
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
