import { useMemo } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { ListBulletIcon, SwatchIcon } from "@heroicons/react/24/outline";
import { Alert, Card, Spinner } from "@heroui/react";
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
import type { LocalOverrideView } from "@pp/client-core";
import { useNavigate } from "react-router-dom";
import { PageShell } from "../../components/PageShell";
import { EntryLinkCard } from "./EntryLinkCard";
import { MasterSwitchCard } from "./MasterSwitchCard";
import { TemplateSection } from "./TemplateSection";

/**
 * 规则主页（ADR-0003 M5.4 拆分，路由 `/rules`，Tab 2）。
 *
 * 自上而下：
 * 1. 总开关卡：`singbox.enabled`（关闭后本地规则与规则集不注入运行配置）；
 * 2. 场景模板：三内置模板应用 / 撤销（J4 起支持自定义模板）；
 * 3. 两个入口卡：自定义规则（`/rules/custom`）与规则集管理（`/rules/rulesets`）。
 *
 * 规则列表 CRUD 与规则集订阅管理已迁至对应子页；规则集启用与否通过
 * `buildSaveInput` 整段透传，本页只消费 `singbox` 桶与 `applied_templates`。
 */
export default function Rules() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  // 本地规则 / 模板在核心启动时注入，运行中变更不热更新：核心运行中成功 toast 追加「重启代理后生效」。
  const coreRunning = status?.core_running ?? false;
  const {
    data: overrideData,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  const currentCore = overrideData ? overrideData.singbox : null;
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  const appliedTemplateIds = useMemo(
    () => new Set(overrideData?.applied_templates.map((t) => t.template_id) ?? []),
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

          {/* 3. 二级页入口 */}
          <EntryLinkCard
            icon={<ListBulletIcon className="size-6" aria-hidden="true" />}
            title="自定义规则"
            description="添加与管理你的分流规则"
            onPress={() => navigate("/rules/custom")}
          />
          <EntryLinkCard
            icon={<SwatchIcon className="size-6" aria-hidden="true" />}
            title="规则集管理"
            description="订阅社区规则集与自定义规则集"
            onPress={() => navigate("/rules/rulesets")}
          />
        </div>
      )}
    </PageShell>
  );
}
