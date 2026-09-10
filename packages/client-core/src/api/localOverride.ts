/**
 * Local override (rule management) API types and functions.
 *
 * Aligned with Rust-side local override views.
 */

import { invoke } from "@tauri-apps/api/core";

export interface LocalRuleView {
  id: string;
  name: string;
  enabled: boolean;
  match_type: string;
  target: string;
  action: string;
  no_resolve: boolean;
  invert: boolean;
  note: string;
  created_at: number;
  sort_order: number;
}

export interface LocalRuleSetRefView {
  id: string;
  name: string;
  tag: string;
  kind: string;
  source: string;
  enabled: boolean;
  auto_update_interval_minutes: number;
  last_updated: number;
}

export interface CoreLocalOverrideView {
  rules: LocalRuleView[];
  rule_sets: LocalRuleSetRefView[];
  enabled: boolean;
}

export interface AppliedTemplateView {
  template_id: string;
  applied_at: number;
  /** 废弃：快照复制时代的产物。引用语义下保留字段仅为 serde/类型兼容（新数据恒空）。 */
  generated_rule_ids: string[];
}

/**
 * Custom rule set source. Mirrors the Rust `CustomRuleSetSource` (internal
 * `kind` tag): `remote` URL + format, or `manual` pasted source JSON.
 */
export type CustomRuleSetSource =
  | { kind: "remote"; url: string; format: "source" | "binary" }
  | { kind: "manual"; content: string };

/** User-defined custom rule set view (from `local_override_get`). */
export interface CustomRuleSetView {
  id: string;
  name: string;
  tag: string;
  source: CustomRuleSetSource;
  /** Local cache write time (Unix seconds; 0 = never downloaded). */
  last_updated: number;
  /**
   * Remote `Last-Modified` (Unix seconds; 0 = unknown). `> last_updated` means
   * the remote has a newer version (frontend "有更新" chip).
   */
  remote_updated_at: number;
  /** Whether the backing file (manual 落盘 / remote cache) exists on disk. */
  cached: boolean;
}

/**
 * Aggregated rule set update outcome (single or batch smart update).
 *
 * `skipped` = remote `Last-Modified` ≤ local `last_updated` (already latest);
 * `failed` = HEAD / download error (best-effort per entry).
 */
export interface RuleSetUpdateOutcome {
  updated: number;
  skipped: number;
  failed: number;
}

/**
 * User-defined scenario template view (from `local_override_get`).
 *
 * `rules` is the template's **rule ID reference list** (not a snapshot).
 * `invalid_count` is computed server-side: references whose rule ID is missing
 * from the rule list or whose rule is disabled.
 */
export interface CustomTemplateView {
  id: string;
  name: string;
  desc: string;
  rules: string[];
  invalid_count: number;
  created_at: number;
}

export interface LocalOverrideView {
  singbox: CoreLocalOverrideView;
  applied_templates: AppliedTemplateView[];
  custom_rule_sets: CustomRuleSetView[];
  custom_templates: CustomTemplateView[];
  /** 用户添加的规则集市场源（源列表 + 缓存条目数）。 */
  market_sources: MarketSourceView[];
}

/** User-added market source (from `local_override_get`). */
export interface MarketSourceView {
  id: string;
  name: string;
  url: string;
  /** Last successful fetch timestamp (Unix seconds; 0 = never). */
  last_fetched: number;
  /** Valid entries in the cached catalog. */
  entry_count: number;
}

/** Market entry (from `local_override_market_entries`), with source origin. */
export interface MarketEntryView {
  id: string;
  name: string;
  description: string;
  category: string;
  format: "source" | "binary";
  url: string;
  source_id: string;
  source_name: string;
}

export interface SaveLocalOverrideInput {
  singbox: CoreLocalOverrideInput;
  applied_templates: AppliedTemplateInput[];
  /** Full-replacement custom rule set segment (same semantics as rules). */
  custom_rule_sets: CustomRuleSetInput[];
  /** Full-replacement custom template segment (same semantics as rules). */
  custom_templates: CustomTemplateInput[];
}

/** Custom rule set save payload (same shape as the view minus `cached`). */
export interface CustomRuleSetInput {
  id: string;
  name: string;
  tag: string;
  source: CustomRuleSetSource;
  last_updated: number;
}

/**
 * Custom template save payload.
 *
 * `rules` is the **rule ID reference list** (only enabled rules are selectable
 * at creation time). A template saved from the current rule list round-trips
 * through `buildSaveInput` unchanged.
 */
export interface CustomTemplateInput {
  id: string;
  name: string;
  desc: string;
  rules: string[];
  created_at: number;
}

export interface CoreLocalOverrideInput {
  rules: LocalRuleInput[];
  rule_sets: LocalRuleSetRefInput[];
  enabled: boolean;
}

export interface LocalRuleInput {
  id: string;
  name: string;
  enabled: boolean;
  match_type: string;
  target: string;
  action: string;
  no_resolve: boolean;
  invert: boolean;
  note: string;
  created_at: number;
  sort_order: number;
}

export interface LocalRuleSetRefInput {
  id: string;
  name: string;
  tag: string;
  kind: string;
  source: string;
  enabled: boolean;
  auto_update_interval_minutes: number;
  last_updated: number;
}

export interface AppliedTemplateInput {
  template_id: string;
  applied_at: number;
  generated_rule_ids: string[];
}

export function localOverrideGet(): Promise<LocalOverrideView> {
  return invoke<LocalOverrideView>("local_override_get");
}

export function localOverrideSave(input: SaveLocalOverrideInput): Promise<void> {
  return invoke<void>("local_override_save", { input });
}

export function localOverrideApplyTemplate(templateId: string): Promise<string[]> {
  return invoke<string[]>("local_override_apply_template", { templateId });
}

export function localOverrideRevertTemplate(templateId: string): Promise<boolean> {
  return invoke<boolean>("local_override_revert_template", { templateId });
}

/** 立即智能更新全部 Remote 规则集（HEAD 比对跳过未变更项）。 */
export function localOverrideUpdateRulesetsNow(): Promise<RuleSetUpdateOutcome> {
  return invoke<RuleSetUpdateOutcome>("local_override_update_rulesets_now");
}

/** 智能更新单个自定义规则集（HEAD 比对跳过未变更项）；返回 1 条计数结果。 */
export function localOverrideUpdateRuleSet(id: string): Promise<RuleSetUpdateOutcome> {
  return invoke<RuleSetUpdateOutcome>("local_override_update_rule_set", { id });
}

/**
 * 添加市场源：后端拉取验证（失败报错且不保存）→ 落盘缓存 → 追加源。
 * 返回新建源的视图（含条目数）。
 */
export function localOverrideMarketAdd(name: string, url: string): Promise<MarketSourceView> {
  return invoke<MarketSourceView>("local_override_market_add", { name, url });
}

/** 删除市场源并清理其缓存文件；返回是否移除了源。 */
export function localOverrideMarketRemove(id: string): Promise<boolean> {
  return invoke<boolean>("local_override_market_remove", { id });
}

/** 刷新单个市场源（重拉 + 落盘 + 更新时间）；返回条目数，失败报错并保留旧缓存。 */
export function localOverrideMarketRefresh(id: string): Promise<number> {
  return invoke<number>("local_override_market_refresh", { id });
}

/** 读取全部源的缓存条目（合并、带来源标注）；纯读缓存，不拉网络。 */
export function localOverrideMarketEntries(): Promise<MarketEntryView[]> {
  return invoke<MarketEntryView[]>("local_override_market_entries");
}
