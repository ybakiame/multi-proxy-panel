import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ArrowPathIcon, PlusIcon } from "@heroicons/react/24/outline";
import { Alert, AlertDialog, Button, Card, Chip, Spinner, Switch } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  buildSaveInput,
  localOverrideGet,
  localOverrideRulesets,
  localOverrideSave,
  localOverrideToggleRuleset,
  localOverrideUpdateRulesetsNow,
  toErrorMessage,
  toastError,
  toastSuccess,
  useProxyStatus,
} from "@pp/client-core";
import type { CustomRuleSetInput, CustomRuleSetView, LocalOverrideView, RuleSetStatusView } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";
import { CustomRuleSetCard } from "./CustomRuleSetCard";
import { RuleSetFormSheet } from "./RuleSetFormSheet";

/**
 * 规则集订阅/缓存状态查询键。
 *
 * 与 `LOCAL_OVERRIDE_KEY` 共用前缀：任何 `invalidateQueries(LOCAL_OVERRIDE_KEY)`
 * （前缀匹配）都会连带重拉本查询；同时避免把 `{override, ruleSets}` 复合形态
 * 写进 `LOCAL_OVERRIDE_KEY`——那是规则主页/自定义规则页黑屏（异构缓存下访问
 * `applied_templates.map` 崩溃）的根因。
 */
const RULE_SETS_STATUS_KEY = [...LOCAL_OVERRIDE_KEY, "rule_sets_status"] as const;

function formatUpdated(lastUpdated: number): string {
  if (lastUpdated <= 0) return "从未更新";
  return new Date(lastUpdated * 1000).toLocaleString();
}

/**
 * 规则集管理子页（ADR-0003 M5.4 拆分，路由 `/rules/rulesets`）。
 *
 * - 顶部「立即更新」：批量下载已订阅社区规则集与远程自定义规则集
 *   （`localOverrideUpdateRulesetsNow`，busy 态，toast 汇总）；
 * - 社区规则集：5 内置卡（订阅 Switch / 缓存 chip / 更新时间），无删除/编辑（模板依赖）；
 * - 自定义规则集：添加/编辑底部 Sheet（远程 URL / 手动 JSON）+ 删除 AlertDialog；
 *   写操作经 `buildSaveInput` 整段替换 `custom_rule_sets` 落盘（J2 链路同步清理文件）；
 *   tag 冲突等 Rust 校验错误 → toast 并保留表单现场。
 */
export default function RuleSetsPage() {
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
  const {
    data: ruleSets,
    isLoading: ruleSetsLoading,
    error: ruleSetsError,
  } = useQuery<RuleSetStatusView[]>({
    queryKey: RULE_SETS_STATUS_KEY,
    queryFn: localOverrideRulesets,
  });

  // 结构守卫：缓存残留异构形态（历史复合查询）时视为未加载，渲染空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const communitySets = asArray(ruleSets);
  const customSets = asArray(overrideData?.custom_rule_sets);
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  // ---- 局部 UI 状态 ----
  const [formOpen, setFormOpen] = useState(false);
  const [editingSet, setEditingSet] = useState<CustomRuleSetView | null>(null);
  const [pendingDelete, setPendingDelete] = useState<CustomRuleSetView | null>(null);
  // 内置订阅 Switch 单飞（同一时刻仅一个进行中）。
  const [togglingCommunityId, setTogglingCommunityId] = useState<string | null>(null);
  // 自定义启停单飞（save 全量落盘，避免同卡并发写）。
  const [togglingCustomId, setTogglingCustomId] = useState<string | null>(null);
  const [updating, setUpdating] = useState(false);

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** 自定义规则集段整段替换落盘（其余段透传当前视图）。 */
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

  // ---- 立即更新 / 社区订阅 ----
  const handleUpdateNow = async (): Promise<boolean> => {
    if (updating) return false;
    setUpdating(true);
    try {
      const updated = await localOverrideUpdateRulesetsNow();
      toastRuleSaved(`已更新 ${updated} 个规则集`);
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

  const handleToggleCommunity = async (ruleSet: RuleSetStatusView, subscribed: boolean): Promise<boolean> => {
    if (togglingCommunityId !== null) return false;
    setTogglingCommunityId(ruleSet.community_id);
    try {
      await localOverrideToggleRuleset(ruleSet.community_id, subscribed);
      toastRuleSaved(subscribed ? `已订阅规则集「${ruleSet.display_name}」` : `已取消订阅「${ruleSet.display_name}」`);
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
      return false;
    } finally {
      setTogglingCommunityId(null);
    }
  };

  // ---- 自定义规则集 增改删启停 ----
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
      const base = exists ? "自定义规则集已更新" : "自定义规则集已添加";
      let text = coreRunning ? `${base}，重启代理后生效` : base;
      if (!exists && input.source.kind === "remote") {
        text += "；点击「立即更新」下载规则集";
      }
      toastSuccess(text);
    }
    return ok;
  };

  const handleToggleCustom = async (ruleSet: CustomRuleSetView, next: boolean) => {
    if (!overrideData || togglingCustomId !== null) return;
    setTogglingCustomId(ruleSet.id);
    const changed = overrideData.custom_rule_sets.map((rs) => (rs.id === ruleSet.id ? { ...rs, enabled: next } : rs));
    try {
      if (await persistCustom(changed)) {
        toastRuleSaved(next ? `已启用自定义规则集「${ruleSet.tag}」` : `已停用自定义规则集「${ruleSet.tag}」`);
      }
    } finally {
      setTogglingCustomId(null);
    }
  };

  const handleDeleteConfirm = async () => {
    const target = pendingDelete;
    if (!target || !overrideData) return;
    setPendingDelete(null);
    const next = overrideData.custom_rule_sets.filter((rs) => rs.id !== target.id);
    if (await persistCustom(next)) {
      toastRuleSaved(`已删除自定义规则集「${target.tag}」`);
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
        {/* 顶部：立即更新 */}
        <div className="flex items-center justify-between gap-2">
          <span className="min-w-0 flex-1 text-xs text-muted">
            订阅的社区规则集与远程自定义规则集需下载后才能被注入
          </span>
          <Button
            variant="secondary"
            className="h-11 shrink-0 px-4"
            isDisabled={updating}
            isPending={updating}
            onPress={() => void handleUpdateNow()}
          >
            <ArrowPathIcon className="size-4" aria-hidden="true" />
            立即更新
          </Button>
        </div>

        {((overrideLoading && !overrideData) || (ruleSetsLoading && ruleSets === undefined)) && (
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

        {overrideData && (
          <>
            {/* 社区规则集（内置，不可删除） */}
            <section className="flex flex-col gap-3">
              <span className="text-sm font-medium text-foreground">社区规则集</span>
              {ruleSetsError && communitySets.length === 0 ? (
                <Card>
                  <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
                    <Alert status="danger">
                      <Alert.Indicator />
                      <Alert.Content>
                        <Alert.Title>订阅状态加载失败</Alert.Title>
                        <Alert.Description>{toErrorMessage(ruleSetsError)}</Alert.Description>
                      </Alert.Content>
                    </Alert>
                  </Card.Content>
                </Card>
              ) : communitySets.length === 0 ? (
                <Card>
                  <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
                    <span className="text-sm text-muted">暂无社区规则集</span>
                  </Card.Content>
                </Card>
              ) : (
                <div className="flex flex-col gap-2">
                  {communitySets.map((ruleSet) => {
                    const toggling = togglingCommunityId === ruleSet.community_id;
                    return (
                      <Card key={ruleSet.community_id}>
                        <div className="flex items-center gap-1 px-2 py-1 pl-0">
                          <div className="flex min-w-0 flex-1 flex-col gap-0.5 px-2 py-2 pl-1">
                            <span className="flex min-w-0 items-center gap-1.5">
                              <span className="min-w-0 truncate text-sm font-medium text-foreground">
                                {ruleSet.display_name}
                              </span>
                              {ruleSet.subscribed ? (
                                <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                                  已订阅
                                </Chip>
                              ) : (
                                <Chip size="sm" variant="soft" color="default" className="shrink-0">
                                  未订阅
                                </Chip>
                              )}
                            </span>
                            <span className="truncate text-xs text-muted">{ruleSet.category}</span>
                            <span className="flex min-w-0 items-center gap-1.5 text-xs text-muted">
                              {ruleSet.singbox_cached ? (
                                <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                                  已缓存
                                </Chip>
                              ) : (
                                <Chip size="sm" variant="soft" color="default" className="shrink-0">
                                  未缓存
                                </Chip>
                              )}
                              <span className="shrink-0">更新于 {formatUpdated(ruleSet.last_updated)}</span>
                            </span>
                          </div>
                          <Switch
                            aria-label={`订阅 ${ruleSet.display_name}`}
                            isSelected={ruleSet.subscribed}
                            isDisabled={toggling}
                            onChange={(next) => void handleToggleCommunity(ruleSet, next)}
                            className="shrink-0 px-1"
                          >
                            <Switch.Content>
                              <Switch.Control>
                                <Switch.Thumb />
                              </Switch.Control>
                            </Switch.Content>
                          </Switch>
                        </div>
                      </Card>
                    );
                  })}
                </div>
              )}
            </section>

            {/* 自定义规则集 */}
            <section className="flex flex-col gap-3">
              <div className="flex items-center justify-between">
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="text-sm font-medium text-foreground">自定义规则集</span>
                  <span className="text-xs text-muted">远程 URL 或手动 JSON · 供本地规则「规则集」匹配引用</span>
                </div>
                <Button variant="primary" className="h-11 shrink-0 px-4" onPress={openAdd}>
                  <PlusIcon className="size-4" aria-hidden="true" />
                  添加
                </Button>
              </div>

              {customSets.length === 0 ? (
                <Card>
                  <Card.Content className="flex flex-col items-center justify-center gap-1 py-8 text-center">
                    <span className="text-sm text-muted">添加你的第一个自定义规则集</span>
                    <span className="text-xs text-muted/80">订阅远程规则集或在本地手写 JSON</span>
                  </Card.Content>
                </Card>
              ) : (
                <div className="flex flex-col gap-2">
                  {customSets.map((ruleSet) => (
                    <CustomRuleSetCard
                      key={ruleSet.id}
                      ruleSet={ruleSet}
                      busy={togglingCustomId === ruleSet.id}
                      onToggle={(next) => void handleToggleCustom(ruleSet, next)}
                      onEdit={() => openEdit(ruleSet)}
                      onDelete={() => setPendingDelete(ruleSet)}
                    />
                  ))}
                </div>
              )}
            </section>
          </>
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
                确定删除自定义规则集「{pendingDelete ? pendingDelete.name.trim() || pendingDelete.tag : ""}」吗？
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
