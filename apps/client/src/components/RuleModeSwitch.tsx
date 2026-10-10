import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Segmented, SegmentedButton, Button, IS_MOBILE } from "@pp/ui";
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
  onError?: (message: string) => void;
  onSuccess?: () => void;
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
export function RuleModeSwitch({ value, running, clashApiEnabled, onError, onSuccess }: RuleModeSwitchProps) {
  const queryClient = useQueryClient();
  // 记录选中即置忙，防止快速连点交错（busy 期间禁用全部选项）。
  const [busy, setBusy] = useState(false);

  const mutation = useMutation({
    mutationFn: (mode: string) => setRuleModeApi(mode),
    onSuccess: (next) => {
      queryClient.setQueryData(PROXY_STATUS_KEY, next);
      void queryClient.invalidateQueries({ queryKey: CONFIG_KEY });
      onSuccess?.();
      toastSuccess("出站模式已切换");
    },
    onError: (err: unknown) => {
      onError?.(toErrorMessage(err));
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
      {IS_MOBILE ? (
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
      ) : (
        <div className="flex items-center gap-2">
          {RULE_MODES.map((mode) => (
            <Button
              key={mode.id}
              size="sm"
              variant={value === mode.id ? "primary" : "secondary"}
              isDisabled={busy}
              isPending={busy && mutation.variables === mode.id}
              onPress={() => handleSelect(mode.id)}
            >
              {mode.label}
            </Button>
          ))}
        </div>
      )}
      <span className="text-xs text-zinc-500 dark:text-zinc-400">
        {running && clashApiEnabled ? "即时生效" : "已保存，将在下次启动生效；运行时切换需开启 Clash API"}
      </span>
    </div>
  );
}
