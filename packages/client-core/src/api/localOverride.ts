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

export interface LocalOverrideView {
  singbox: CoreLocalOverrideView;
  custom_rule_sets: CustomRuleSetView[];
}

export interface SaveLocalOverrideInput {
  singbox: CoreLocalOverrideInput;
  /** Full-replacement custom rule set segment (same semantics as rules). */
  custom_rule_sets: CustomRuleSetInput[];
}

/** Custom rule set save payload (same shape as the view minus `cached`). */
export interface CustomRuleSetInput {
  id: string;
  name: string;
  tag: string;
  source: CustomRuleSetSource;
  last_updated: number;
  /**
   * Remote modification time (Unix seconds; 0 = unknown). Backend preserves it
   * for new IDs and backfills existing IDs from disk.
   */
  remote_updated_at?: number;
}

export interface CoreLocalOverrideInput {
  rules: LocalRuleInput[];
  rule_sets: LocalRuleSetRefInput[];
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

export function localOverrideGet(): Promise<LocalOverrideView> {
  return invoke<LocalOverrideView>("local_override_get");
}

export function localOverrideSave(input: SaveLocalOverrideInput): Promise<void> {
  return invoke<void>("local_override_save", { input });
}

/** 立即智能更新全部 Remote 规则集（HEAD 比对跳过未变更项）。 */
export function localOverrideUpdateRulesetsNow(): Promise<RuleSetUpdateOutcome> {
  return invoke<RuleSetUpdateOutcome>("local_override_update_rulesets_now");
}

/** 智能更新单个自定义规则集（HEAD 比对跳过未变更项）；返回 1 条计数结果。 */
export function localOverrideUpdateRuleSet(id: string): Promise<RuleSetUpdateOutcome> {
  return invoke<RuleSetUpdateOutcome>("local_override_update_rule_set", { id });
}
