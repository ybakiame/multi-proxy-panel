/** 出站模式选项与标签（与后端 `rule` / `global` / `direct` 对齐，见 types.ts 的 `rule_mode`）。 */
export const RULE_MODES = [
  { id: "rule", label: "规则" },
  { id: "global", label: "全局" },
  { id: "direct", label: "直连" },
] as const;

export type RuleModeId = (typeof RULE_MODES)[number]["id"];

export const RULE_MODE_LABELS: Record<string, string> = Object.fromEntries(
  RULE_MODES.map((mode) => [mode.id, mode.label]),
);
