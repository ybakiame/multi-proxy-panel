import { Alert } from "@heroui/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { LOCAL_OVERRIDE_KEY, localOverrideGet, toErrorMessage, useProxyStatus } from "@pp/client-core";
import type { LocalOverrideView } from "@pp/client-core";
import { RuleSetSection } from "./RuleSetSection";

/**
 * 规则集管理子页（桌面端，路由 `/config/route/rulesets`；语义对齐移动端
 * `Config/RuleSetsPage`，数据键同为 `local_override.json`）。
 *
 * 规则集是纯资源（无 enabled）：是否注入由引用它的规则（规则管理页）决定。
 * 主体复用 [`RuleSetSection`]（社区 / 自定义单一表格形态）。
 */
export default function RuleSets() {
  const queryClient = useQueryClient();
  const { data: status } = useProxyStatus();
  const coreRunning = status?.core_running ?? false;

  const {
    data: overrideData,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });
  const error = queryError ? toErrorMessage(queryError) : null;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">规则集管理</h1>
        <p className="text-sm text-muted">社区与自定义规则集；经规则的 rule_set 目标引用后生效</p>
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
          <span className="text-sm text-muted">正在加载规则集配置…</span>
        </div>
      )}

      {overrideData && <RuleSetSection overrideData={overrideData} coreRunning={coreRunning} onChanged={invalidate} />}
    </div>
  );
}
