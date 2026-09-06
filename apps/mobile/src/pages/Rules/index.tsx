import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Alert, Card, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  buildSaveInput,
  localOverrideApplyTemplate,
  localOverrideGet,
  localOverrideRevertTemplate,
  localOverrideRulesets,
  localOverrideSave,
  localOverrideToggleRuleset,
  localOverrideUpdateRulesetsNow,
  ruleSummary,
  toErrorMessage,
  toastError,
  toastSuccess,
  viewToInput,
} from "@pp/client-core";
import type {
  CoreLocalOverrideInput,
  LocalOverrideView,
  LocalRuleInput,
  LocalRuleView,
  RuleSetStatusView,
} from "@pp/client-core";
import { PageShell } from "../../components/PageShell";
import { MasterSwitchCard } from "./MasterSwitchCard";
import { RuleDeleteConfirm } from "./RuleDeleteConfirm";
import { RuleEditSheet } from "./RuleEditSheet";
import { RuleListSection } from "./RuleListSection";
import { RuleSetsSection } from "./RuleSetsSection";
import { TemplateSection } from "./TemplateSection";

interface RulesData {
  override: LocalOverrideView;
  ruleSets: RuleSetStatusView[];
}

/** 规则页一次取数：本地 Override 全量 + 规则集订阅状态（对齐 desktop Rules 的 fetchRulesData）。 */
async function fetchRulesData(): Promise<RulesData> {
  const [override, ruleSets] = await Promise.all([localOverrideGet(), localOverrideRulesets()]);
  return { override, ruleSets };
}

/**
 * 规则管理页（ADR-0003 M5.4，原占位页重写为完整页）。四区自上而下：
 *
 * 1. 总开关卡：`singbox.enabled`（关闭后本地规则与规则集不注入运行配置）；
 * 2. 场景模板：`TEMPLATE_DEFS` 三卡（回国/海外/广告过滤）——应用
 *    `localOverrideApplyTemplate` / 撤销 `localOverrideRevertTemplate`；
 * 3. 自定义规则：卡片（ruleSummary + 动作 badge + 启用 Switch + 点卡编辑）+
 *    上移/下移（对齐 desktop sort_order 重排）+ 添加（编辑 Sheet）；
 * 4. 规则集：社区规则集订阅管理（订阅 Switch + 立即更新）。
 *
 * 全部数据经 @pp/client-core：mutation 成功后失效 LOCAL_OVERRIDE_KEY（本地规则与
 * 规则集状态同属一个 query，一并重读）；规则增删改/启停/排序走
 * `persist = buildSaveInput + localOverrideSave` 全量落盘（对齐 desktop Rules），
 * 成功后按动作差异化 toast。
 */
export default function Rules() {
  const queryClient = useQueryClient();
  const {
    data,
    isLoading,
    error: queryError,
  } = useQuery<RulesData>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: fetchRulesData,
  });

  const overrideData = data?.override ?? null;
  const ruleSets = data?.ruleSets ?? [];
  // 单核心（sing-box）：直接消费 singbox 桶。
  const currentCore = overrideData ? overrideData.singbox : null;

  // ---- 局部 UI 状态 ----
  const [editOpen, setEditOpen] = useState(false);
  const [editingRule, setEditingRule] = useState<LocalRuleView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<LocalRuleView | null>(null);

  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  const appliedTemplateIds = useMemo(
    () => new Set(overrideData?.applied_templates.map((t) => t.template_id) ?? []),
    [overrideData],
  );

  /** 规则写操作统一落盘：全量 patch（buildSaveInput + localOverrideSave），失败 toast；返回是否成功。 */
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

  // ---- 总开关 / 规则列表写操作（persist 模式） ----
  const handleToggleEnabled = async (next: boolean) => {
    if (!currentCore) return;
    if (await persist({ ...viewToInput(currentCore), enabled: next })) {
      toastSuccess(next ? "本地规则已启用" : "本地规则已关闭");
    }
  };

  const handleToggleRule = async (rule: LocalRuleView, next: boolean) => {
    if (!currentCore) return;
    const nextRules = currentCore.rules.map((r) => (r.id === rule.id ? { ...r, enabled: next } : r));
    if (await persist({ ...viewToInput(currentCore), rules: nextRules })) {
      toastSuccess(next ? `已启用规则「${ruleSummary(rule)}」` : `已停用规则「${ruleSummary(rule)}」`);
    }
  };

  /** 上移 / 下移（dir = -1 | 1），重排后按新位置回写 sort_order（对齐 desktop handleMoveUp/Down）。 */
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
      toastSuccess("规则顺序已更新");
    }
  };

  /** 新增/编辑共用（对齐 desktop handleSaveRule：已存在替换，否则追加并赋予末尾 sort_order）。 */
  const handleSaveRule = async (rule: LocalRuleInput): Promise<boolean> => {
    if (!currentCore) return false;
    const exists = currentCore.rules.some((r) => r.id === rule.id);
    const nextRules = exists
      ? currentCore.rules.map((r) => (r.id === rule.id ? rule : r))
      : [...currentCore.rules, { ...rule, sort_order: currentCore.rules.length }];
    const ok = await persist({ ...viewToInput(currentCore), rules: nextRules });
    if (ok) {
      toastSuccess(exists ? "规则已更新" : "规则已添加");
    }
    return ok;
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target || !currentCore) return;
    setPendingDelete(null);
    const nextRules = currentCore.rules.filter((r) => r.id !== target.id);
    if (await persist({ ...viewToInput(currentCore), rules: nextRules })) {
      toastSuccess("规则已删除");
    }
  };

  // ---- 场景模板（独立 mutation，成功后失效重读） ----
  const handleApplyTemplate = async (templateId: string): Promise<boolean> => {
    try {
      await localOverrideApplyTemplate(templateId);
      toastSuccess("模板已应用");
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
    }
  };

  const handleRevertTemplate = async (templateId: string): Promise<boolean> => {
    try {
      await localOverrideRevertTemplate(templateId);
      toastSuccess("模板已撤销");
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
    }
  };

  // ---- 规则集订阅 / 立即更新 ----
  const handleToggleRuleset = async (communityId: string, subscribed: boolean): Promise<boolean> => {
    try {
      await localOverrideToggleRuleset(communityId, subscribed);
      toastSuccess(subscribed ? "已订阅规则集" : "已取消订阅规则集");
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
    }
  };

  const handleUpdateRulesetsNow = async (): Promise<boolean> => {
    try {
      const updated = await localOverrideUpdateRulesetsNow();
      toastSuccess(`已更新 ${updated} 个规则集`);
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
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
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">规则管理</h1>
        <p className="text-sm text-muted">本地规则 · 场景模板 · 规则集订阅</p>
      </div>

      {queryError && (
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

      {isLoading && !overrideData && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-muted">正在加载规则配置…</span>
          </Card.Content>
        </Card>
      )}

      {overrideData && currentCore && (
        <div className="flex flex-col gap-4">
          {/* 1. 总开关 */}
          <MasterSwitchCard enabled={currentCore.enabled} onToggle={(next) => void handleToggleEnabled(next)} />

          {/* 2. 场景模板 */}
          <TemplateSection
            appliedIds={appliedTemplateIds}
            onApply={(id) => handleApplyTemplate(id)}
            onRevert={(id) => handleRevertTemplate(id)}
          />

          {/* 3. 自定义规则列表 */}
          <RuleListSection
            rules={currentCore.rules}
            onToggle={(rule, next) => void handleToggleRule(rule, next)}
            onMove={(index, dir) => void handleMoveRule(index, dir)}
            onEdit={(rule) => openEdit(rule)}
            onAdd={openAdd}
          />

          {/* 4. 规则集订阅管理 */}
          <RuleSetsSection
            ruleSets={ruleSets}
            onToggle={(id, subscribed) => handleToggleRuleset(id, subscribed)}
            onUpdateNow={() => handleUpdateRulesetsNow()}
          />
        </div>
      )}

      {/* 编辑 Sheet 与删除确认（常驻挂载，isOpen / rule 控制显隐） */}
      <RuleEditSheet
        isOpen={editOpen}
        editing={editingRule}
        onClose={() => setEditOpen(false)}
        onSave={(rule) => handleSaveRule(rule)}
        onDeleteRequest={(rule) => handleDeleteRequest(rule)}
      />
      <RuleDeleteConfirm
        rule={pendingDelete}
        onClose={() => setPendingDelete(null)}
        onConfirm={() => void handleDeleteConfirm()}
      />
    </PageShell>
  );
}
