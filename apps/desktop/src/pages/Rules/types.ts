/**
 * 规则纯函数自 desktop 上移共享库 @pp/client-core（ADR-0003 M5.4）。
 *
 * 本文件保留为 re-export 垫片：保持 Rules 页各组件 `from "./types"` 的导入路径
 * 不变（最小 diff），实现统一收敛到 @pp/client-core 的 rules 模块。
 */
export {
  MATCH_TYPE_LABELS,
  ACTION_LABELS,
  TEMPLATE_DEFS,
  RULE_ACTIONS,
  matchTypeLabel,
  actionLabel,
  ruleSummary,
  ruleDetailLine,
  viewToInput,
  buildSaveInput,
} from "@pp/client-core";
