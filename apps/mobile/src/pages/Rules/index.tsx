import { useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ListBulletIcon, SwatchIcon } from "@heroicons/react/24/outline";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  buildSaveInput,
  localOverrideApplyTemplate,
  localOverrideGet,
  localOverrideRevertTemplate,
  localOverrideSave,
  toErrorMessage,
  toastError,
  toastSuccess,
  useProxyStatus,
  viewToInput,
} from "@pp/client-core";
import type { CustomTemplateInput, CustomTemplateView, LocalOverrideView } from "@pp/client-core";
import { useNavigate } from "react-router-dom";
import { EntryLinkCard } from "../../components/EntryLinkCard";
import { PageShell } from "../../components/PageShell";
import { isLocalOverrideView } from "./localOverrideGuards";
import { MasterSwitchCard } from "./MasterSwitchCard";
import { TemplateSection } from "./TemplateSection";

/**
 * 规则主页（ADR-0003 M5.4 拆分，路由 `/rules`，Tab 2）。
 *
 * 自上而下：
 * 1. 总开关卡：`singbox.enabled`（关闭后本地规则与规则集不注入运行配置）；
 * 2. 二级页入口：规则集管理（`/rules/rulesets`）与自定义规则（`/rules/custom`）；
 * 3. 场景模板：自定义模板应用 / 撤销与新建（内置模板已废弃）。
 *
 * 自「模板改为规则 ID 引用 + 应用激活」起：模板保存的是规则 ID 引用（不复制规则），
 * 应用/撤销只是激活/停用场景，只有被已应用模板引用的启用规则才注入启动配置。
 * 规则列表 CRUD 与规则集管理已迁至对应子页；本页只消费 `singbox` 桶、
 * `applied_templates` 与 `custom_templates`（custom 段由 `buildSaveInput` 整段透传）。
 */
export default function Rules() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 本地规则 / 模板在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  const {
    data: rawOverride,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  // 结构守卫（见 localOverrideGuards.ts）：缓存中残留异构形态（如历史版本规则集
  // 管理页写入的 { override, ruleSets }）一律视为未加载 → 渲染可恢复的空态，避免
  // 访问 undefined 字段（如 applied_templates.map）导致整页崩溃黑屏。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const currentCore = overrideData ? overrideData.singbox : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  const appliedTemplateIds = useMemo(
    () => new Set((overrideData?.applied_templates ?? []).map((t) => t.template_id)),
    [overrideData],
  );

  const toastRuleSaved = (base: string) => {
    toastSuccess(coreRunning ? `${base}，重启代理后生效` : base);
  };

  /** 总开关写操作：全量 patch（buildSaveInput + localOverrideSave），失败 toast。 */
  const handleToggleEnabled = async (next: boolean) => {
    if (!overrideData || !currentCore) return;
    try {
      await localOverrideSave(buildSaveInput(overrideData, { ...viewToInput(currentCore), enabled: next }));
      toastRuleSaved(next ? "本地规则已启用" : "本地规则已关闭");
      invalidate();
    } catch (err) {
      toastError(toErrorMessage(err));
    }
  };

  // ---- 场景模板（独立 mutation，成功后失效重读） ----
  const handleApplyTemplate = async (templateId: string): Promise<boolean> => {
    try {
      await localOverrideApplyTemplate(templateId);
      toastRuleSaved("模板已应用");
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
      toastRuleSaved("模板已撤销");
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      return false;
    }
  };

  // ---- 自定义场景模板（custom_templates 段追加/移除落盘） ----
  const persistTemplates = async (next: CustomTemplateView[]): Promise<boolean> => {
    if (!overrideData) return false;
    try {
      await localOverrideSave({
        ...buildSaveInput(overrideData),
        // 落盘走 Input 形态：去掉只读的 invalid_count（服务端下次读取时按引用重算）。
        custom_templates: next.map((t) => ({
          id: t.id,
          name: t.name,
          desc: t.desc,
          rules: t.rules,
          created_at: t.created_at,
        })),
      });
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
      return false;
    }
  };

  const handleCreateTemplate = async (template: CustomTemplateInput): Promise<boolean> => {
    if (!overrideData) return false;
    // 新建来源均为当前已启用的规则，本地引用必然有效 → invalid_count 暂按 0，
    // 之后 disable/删除源规则产生的失效引用由服务端读取时重新计算。
    const nextView: CustomTemplateView = { ...template, invalid_count: 0 };
    const ok = await persistTemplates([...overrideData.custom_templates, nextView]);
    if (ok) {
      toastRuleSaved("场景模板已保存");
    }
    return ok;
  };

  const handleDeleteTemplate = async (template: CustomTemplateView): Promise<boolean> => {
    if (!overrideData) return false;
    const next = overrideData.custom_templates.filter((t) => t.id !== template.id);
    const ok = await persistTemplates(next);
    if (ok) {
      toastRuleSaved(`已删除模板「${template.name.trim() || template.id}」`);
    }
    return ok;
  };

  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">规则管理</h1>
        <p className="text-sm text-muted">本地规则 · 场景模板 · 自定义规则集</p>
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

      {!isLoading && !queryError && !overrideData && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
            <span className="text-sm text-muted">规则数据不可用</span>
            <Button variant="secondary" className="min-h-10 shrink-0 px-4" onPress={() => void invalidate()}>
              重新加载
            </Button>
          </Card.Content>
        </Card>
      )}

      {overrideData && currentCore && (
        <div className="flex flex-col gap-4">
          {/* 1. 总开关 */}
          <MasterSwitchCard enabled={currentCore.enabled} onToggle={(next) => void handleToggleEnabled(next)} />

          {/* 2. 二级页入口：规则集管理（社区/自定义分区）→ 自定义规则 */}
          <EntryLinkCard
            icon={<SwatchIcon className="size-6" aria-hidden="true" />}
            title="规则集管理"
            description="社区与自定义规则集的增删与更新"
            onPress={() => navigate("/rules/rulesets")}
          />
          <EntryLinkCard
            icon={<ListBulletIcon className="size-6" aria-hidden="true" />}
            title="自定义规则"
            description="添加与管理你的分流规则"
            onPress={() => navigate("/rules/custom")}
          />

          {/* 3. 场景模板（自定义） */}
          <TemplateSection
            appliedIds={appliedTemplateIds}
            customTemplates={overrideData.custom_templates}
            // 新建模板只从已启用规则中勾选（引用语义：禁用规则不参与注入）。
            ruleOptions={currentCore.rules.filter((r) => r.enabled)}
            onApply={(id) => handleApplyTemplate(id)}
            onRevert={(id) => handleRevertTemplate(id)}
            onCreate={(template) => handleCreateTemplate(template)}
            onDelete={(template) => handleDeleteTemplate(template)}
          />
        </div>
      )}
    </PageShell>
  );
}
