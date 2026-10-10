import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Segmented, SegmentedButton } from "@pp/ui";
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
      <span className="text-sm font-medium text-zinc-900 dark:text-zinc-100">出站模式</span>
      <Segmented strong>
        {RULE_MODES.map((mode) => (
          <SegmentedButton
            key={mode.id}
            active={value === mode.id}
            disabled={busy}
            onClick={() => void handleSelect(mode.id)}
            className="min-h-11"
          >
            {mode.label}
          </SegmentedButton>
        ))}
      </Segmented>
      <span className="text-xs text-zinc-500 dark:text-zinc-400">{running && clashApiEnabled ? "" : ""}</span>
    </div>
  );
}
