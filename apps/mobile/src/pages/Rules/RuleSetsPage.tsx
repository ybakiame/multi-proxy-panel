import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon, PlusIcon, SparklesIcon } from "@heroicons/react/24/outline";
import { Alert, AlertDialog, Button, Card, Chip, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  buildSaveInput,
  localOverrideGet,
  localOverrideSave,
  localOverrideUpdateRuleSet,
  localOverrideUpdateRulesetsNow,
  toErrorMessage,
  toastError,
  toastSuccess,
  toastWarning,
  useProxyStatus,
} from "@pp/client-core";
import type { CustomRuleSetInput, CustomRuleSetView, LocalOverrideView } from "@pp/client-core";
import { useNavigate } from "react-router-dom";
import { BackHeader } from "../../components/BackHeader";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";
import { CustomRuleSetCard } from "./CustomRuleSetCard";
import { RuleSetFormSheet } from "./RuleSetFormSheet";

/** 分区渲染描述：社区 = 远程 URL（remote），自定义 = 手动 JSON（manual）。 */
interface RuleSetSectionSpec {
  /** 分区标题（按来源类型命名）。 */
  title: string;
  /** 空分区引导文案。 */
  emptyCopy: string;
  sets: CustomRuleSetView[];
}

interface RuleSetSectionProps extends RuleSetSectionSpec {
  /** 正在单卡更新的规则集 id（null = 无）。 */
  updatingId: string | null;
  onEdit: (ruleSet: CustomRuleSetView) => void;
  onDelete: (ruleSet: CustomRuleSetView) => void;
  onUpdate: (ruleSet: CustomRuleSetView) => void;
}

/**
 * 单来源分区（社区 / 自定义）：区头标题 + 计数，条目为 `CustomRuleSetCard`；
 * 空分区渲染简短引导，引导新用户走顶部「添加」入口。
 */
function RuleSetSection({ title, emptyCopy, sets, updatingId, onEdit, onDelete, onUpdate }: RuleSetSectionProps) {
  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <span className="min-w-0 truncate text-sm font-medium text-foreground">{title}</span>
        <Chip size="sm" variant="soft" color="default" className="shrink-0">
          {sets.length}
        </Chip>
      </div>
      {sets.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center px-6 py-8 text-center">
            <span className="text-sm text-muted">{emptyCopy}</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {sets.map((ruleSet) => (
            <CustomRuleSetCard
              key={ruleSet.id}
              ruleSet={ruleSet}
              updating={updatingId === ruleSet.id}
              onEdit={() => onEdit(ruleSet)}
              onDelete={() => onDelete(ruleSet)}
              onUpdate={() => onUpdate(ruleSet)}
            />
          ))}
        </div>
      )}
    </section>
  );
}

/**
 * 规则集管理子页（ADR-0003 M5.4 拆分，路由 `/rules/rulesets`）。
 *
 * 自「废弃内置规则集订阅」起页面只管理**用户自控的规则集**，并按**来源类型**分区：
 * - 「社区规则集」区：`source.kind === "remote"`（远程 URL，如 geoip/geosite 社区资源）；
 * - 「自定义规则集」区：`source.kind === "manual"`（手动 JSON）。
 *
 * 自「规则集移除 enabled」起规则集是**纯资源**：
 * - 卡片不再有启停 Switch（名称 / tag / 来源 chip / 缓存状态 / 本地与远程更新时间 / 编辑 / 删除）；
 * - 顶部「立即更新」智能更新**全部** Remote（`localOverrideUpdateRulesetsNow`：先 HEAD
 *   比对 `Last-Modified` 跳过未变更项），toast 汇总「更新 N，已最新 M，失败 K」；
 * - 每张 Remote 卡片另有单卡更新按钮（`localOverrideUpdateRuleSet`），同样智能跳过；
 * - invalidate 触发重拉即见 cached / last_updated / remote_updated_at 变化；
 * - 是否注入仍由引用它的规则决定（引用它的规则被注入时才注入该规则集）。
 */
export default function RuleSetsPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 本地规则 / 规则集在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  const {
    data: rawOverride,
    isLoading: overrideLoading,
    error: overrideError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  // 结构守卫：缓存残留异构形态（历史复合查询）时视为未加载，渲染空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const customSets = asArray(overrideData?.custom_rule_sets);
  // 分区 = 来源类型：remote（社区）与 manual（自定义）。
  const remoteSets = customSets.filter((rs) => rs.source.kind === "remote");
  const manualSets = customSets.filter((rs) => rs.source.kind === "manual");
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  // ---- 局部 UI 状态 ----
  const [formOpen, setFormOpen] = useState(false);
  const [editingSet, setEditingSet] = useState<CustomRuleSetView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<CustomRuleSetView | null>(null);
  const [updating, setUpdating] = useState(false);
  const [updatingId, setUpdatingId] = useState<string | null>(null);

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** 规则集段整段替换落盘（其余段透传当前视图）。 */
  const persistCustom = async (next: CustomRuleSetInput[]): Promise<boolean> => {
    if (!overrideData) return false;
    try {
      await localOverrideSave({ ...buildSaveInput(overrideData), custom_rule_sets: next });
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
      return false;
    }
  };

  // ---- 立即更新（全部 Remote，智能跳过）：返回 updated / skipped / failed 汇总 ----
  const handleUpdateNow = async (): Promise<boolean> => {
    if (updating) return false;
    if (remoteSets.length === 0) {
      toastWarning("暂无远程规则集可更新");
      return true;
    }
    setUpdating(true);
    try {
      const { updated, skipped, failed } = await localOverrideUpdateRulesetsNow();
      const suffix = coreRunning ? "，重启代理后生效" : "";
      const summary = `更新 ${updated}，已最新 ${skipped}，失败 ${failed}${suffix}`;
      if (failed > 0) {
        toastWarning(summary);
      } else {
        toastSuccess(summary);
      }
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
      return false;
    } finally {
      setUpdating(false);
    }
  };

  // ---- 单卡更新（同样智能跳过）：updated=已更新 / skipped=已是最新 / failed=失败 ----
  const handleUpdateOne = async (ruleSet: CustomRuleSetView): Promise<void> => {
    if (updatingId !== null) return;
    setUpdatingId(ruleSet.id);
    const label = ruleSet.name.trim() || ruleSet.tag;
    try {
      const { updated, skipped } = await localOverrideUpdateRuleSet(ruleSet.id);
      if (updated > 0) {
        toastSuccess(coreRunning ? `规则集「${label}」已更新，重启代理后生效` : `规则集「${label}」已更新`);
      } else if (skipped > 0) {
        toastSuccess(`规则集「${label}」已是最新`);
      } else {
        toastWarning(`规则集「${label}」更新失败`);
      }
      invalidate();
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
    } finally {
      setUpdatingId(null);
    }
  };

  // ---- 规则集 增改删 ----
  const openAdd = () => {
    setEditingSet(null);
    setFormOpen(true);
  };

  const openEdit = (ruleSet: CustomRuleSetView) => {
    setEditingSet(ruleSet);
    setFormOpen(true);
  };

  const handleSaveCustom = async (input: CustomRuleSetInput): Promise<boolean> => {
    if (!overrideData) return false;
    const exists = overrideData.custom_rule_sets.some((rs) => rs.id === input.id);
    const next = exists
      ? overrideData.custom_rule_sets.map((rs) => (rs.id === input.id ? input : rs))
      : [...overrideData.custom_rule_sets, input];
    const ok = await persistCustom(next);
    if (ok) {
      const base = exists ? "规则集已更新" : "规则集已添加";
      let text = coreRunning ? `${base}，重启代理后生效` : base;
      if (!exists && input.source.kind === "remote") {
        text += "；点击「立即更新」下载规则集";
      }
      toastSuccess(text);
    }
    return ok;
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target || !overrideData) return;
    setPendingDelete(null);
    const next = overrideData.custom_rule_sets.filter((rs) => rs.id !== target.id);
    if (await persistCustom(next)) {
      toastRuleSaved(`已删除规则集「${target.tag}」`);
    }
  };

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="规则集管理" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {overrideLoading && !overrideData && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在加载规则集…</span>
            </Card.Content>
          </Card>
        )}

        {!overrideData && !overrideLoading && overrideError && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>加载失败</Alert.Title>
                  <Alert.Description>{toErrorMessage(overrideError)}</Alert.Description>
                </Alert.Content>
              </Alert>
            </Card.Content>
          </Card>
        )}

        {!overrideData && !overrideLoading && !overrideError && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
              <span className="text-sm text-muted">规则集数据不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => void invalidate()}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        {overrideData && (
          <div className="flex flex-col gap-4">
            {/* 顶部说明 */}
            <span className="text-xs text-muted">
              远程 URL（社区）规则集需「立即更新」下载后才会被注入；手动 JSON（自定义）保存即写入本地文件
            </span>

            {/* 顶部统一操作：添加（单一入口）+ 立即更新 */}
            <div className="flex items-center gap-2">
              <Button variant="primary" className="h-11 min-w-0 flex-1 px-4" onPress={openAdd}>
                <PlusIcon className="size-4" aria-hidden="true" />
                添加
              </Button>
              <Button
                variant="secondary"
                className="h-11 min-w-0 flex-1 px-4"
                isDisabled={updating}
                isPending={updating}
                onPress={() => void handleUpdateNow()}
              >
                <ArrowPathIcon className="size-4" aria-hidden="true" />
                立即更新
              </Button>
              <Button
                variant="secondary"
                className="h-11 shrink-0 px-3"
                onPress={() => navigate("/rules/rulesets/market")}
              >
                <SparklesIcon className="size-4" aria-hidden="true" />
                市场
              </Button>
            </div>

            {/* 社区规则集：远程 URL（remote） */}
            <RuleSetSection
              title="社区规则集"
              emptyCopy="添加远程 URL 规则集，如 geoip/geosite 社区资源"
              sets={remoteSets}
              updatingId={updatingId}
              onEdit={openEdit}
              onDelete={setPendingDelete}
              onUpdate={(ruleSet) => void handleUpdateOne(ruleSet)}
            />

            {/* 自定义规则集：手动 JSON（manual） */}
            <RuleSetSection
              title="自定义规则集"
              emptyCopy="手动输入 JSON 规则内容"
              sets={manualSets}
              updatingId={updatingId}
              onEdit={openEdit}
              onDelete={setPendingDelete}
              onUpdate={(ruleSet) => void handleUpdateOne(ruleSet)}
            />
          </div>
        )}
      </div>

      <RuleSetFormSheet
        isOpen={formOpen}
        editing={editingSet}
        onClose={() => setFormOpen(false)}
        onSave={(ruleSet) => handleSaveCustom(ruleSet)}
      />
      <AlertDialog.Backdrop
        isOpen={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除规则集</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">
                确定删除规则集「{pendingDelete ? pendingDelete.name.trim() || pendingDelete.tag : ""}」吗？
                将同时清理其缓存文件；引用该 tag 的规则需一并调整。
              </p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={() => void handleDeleteConfirm()}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </div>
  );
}
