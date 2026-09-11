import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "@heroui/react";
import {
  CONFIG_KEY,
  PROXY_STATUS_KEY,
  setRuleMode as setRuleModeApi,
  toErrorMessage,
  toastError,
  toastSuccess,
} from "@pp/client-core";
import { RULE_MODES } from "./ruleModes";

interface RuleModeSwitchProps {
  /** 当前生效模式（运行状态优先，其次配置）。 */
  value: string;
  /** 核心是否运行。 */
  running: boolean;
  /** Clash API 是否开启（决定「即时生效」提示）。 */
  clashApiEnabled: boolean;
}

/**
 * 出站模式分段控件（ADR-0003 M5），作为 `StatusCard` 的内嵌部分渲染（无独立卡片）。
 *
 * - `set_rule_mode` mutation，成功后用返回的最新运行状态回写 PROXY_STATUS_KEY
 *   （对齐 desktop `ruleModeMutation` 模式）+ toast；
 * - 提示：运行中且 Clash API 开启 →「即时生效」，否则「将在下次启动生效」。
 */
export function RuleModeSwitch({ value, running, clashApiEnabled }: RuleModeSwitchProps) {
  const queryClient = useQueryClient();
  // 记录选中即置忙，防止快速连点交错（busy 期间禁用全部选项）。
  const [busy, setBusy] = useState(false);

  const mutation = useMutation({
    mutationFn: (mode: string) => setRuleModeApi(mode),
    onSuccess: (next) => {
      queryClient.setQueryData(PROXY_STATUS_KEY, next);
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      toastSuccess("出站模式已切换");
    },
    onError: (err: unknown) => {
      toastError(toErrorMessage(err));
    },
  });

  const handleSelect = (mode: string) => {
    if (busy || mode === value) {
      return;
    }
    setBusy(true);
    mutation.mutate(mode, {
      onSettled: () => setBusy(false),
    });
  };

  return (
    <div className="flex flex-col gap-2">
      <fieldset className="flex min-w-0 gap-1 rounded-xl border border-border/60 bg-surface-secondary/40 p-1">
        <legend className="sr-only">出站模式</legend>
        {RULE_MODES.map((mode) => (
          <Button
            key={mode.id}
            variant={value === mode.id ? "primary" : "secondary"}
            size="sm"
            className="min-h-11 flex-1"
            isDisabled={busy}
            onPress={() => void handleSelect(mode.id)}
          >
            {mode.label}
          </Button>
        ))}
      </fieldset>
      <span className="text-xs text-muted">
        {running && clashApiEnabled ? "即时生效（依赖 Clash API）" : "将在下次启动生效"}
      </span>
    </div>
  );
}
