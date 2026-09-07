/**
 * 规则纯函数自 desktop 上移共享库 @pp/client-core（ADR-0003 M5.4）。
 *
 * 本文件保留为 re-export 垫片：保持 Rules 页各组件 `from "./types"` 的导入路径
 * 不变（最小 diff），实现统一收敛到 @pp/client-core 的 rules 模块。
 *
 * 自「废弃内置规则集订阅与内置场景模板」起不再 re-export `TEMPLATE_DEFS`
 * （desktop 规则页已移除内置模板区，后续批次再做自定义模板 UI）。
 */
export {
  MATCH_TYPE_LABELS,
  ACTION_LABELS,
  RULE_ACTIONS,
  matchTypeLabel,
  actionLabel,
  ruleSummary,
  ruleDetailLine,
  viewToInput,
  buildSaveInput,
} from "@pp/client-core";
