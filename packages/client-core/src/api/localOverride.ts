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

export interface RuleSetSubscriptionView {
  id: string;
  community_id: string;
  display_name: string;
  category: string;
  subscribed: boolean;
  singbox_url_template: string;
  default_interval_minutes: number;
}

export interface AppliedTemplateView {
  template_id: string;
  applied_at: number;
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
  enabled: boolean;
  last_updated: number;
  /** Whether the backing file (manual 落盘 / remote cache) exists on disk. */
  cached: boolean;
}

/**
 * User-defined scenario template view (from `local_override_get`).
 *
 * `rules` is the template's snapshot of the selected rule cards at creation
 * time (displayed back in the create form and used as the apply source).
 */
export interface CustomTemplateView {
  id: string;
  name: string;
  desc: string;
  rules: LocalRuleView[];
  created_at: number;
}

export interface LocalOverrideView {
  singbox: CoreLocalOverrideView;
  rule_set_subscriptions: RuleSetSubscriptionView[];
  applied_templates: AppliedTemplateView[];
  custom_rule_sets: CustomRuleSetView[];
  custom_templates: CustomTemplateView[];
}

export interface RuleSetStatusView {
  id: string;
  community_id: string;
  display_name: string;
  category: string;
  subscribed: boolean;
  singbox_cached: boolean;
  last_updated: number;
}

export interface SaveLocalOverrideInput {
  singbox: CoreLocalOverrideInput;
  rule_set_subscriptions: RuleSetSubscriptionInput[];
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
  enabled: boolean;
  last_updated: number;
}

/**
 * Custom template save payload.
 *
 * `rules` is a full snapshot of the selected rule cards. `LocalRuleView` and
 * `LocalRuleInput` share the same field shape, so a template saved from the
 * current rule list round-trips through `buildSaveInput` unchanged.
 */
export interface CustomTemplateInput {
  id: string;
  name: string;
  desc: string;
  rules: LocalRuleInput[];
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

export interface RuleSetSubscriptionInput {
  id: string;
  community_id: string;
  display_name: string;
  category: string;
  subscribed: boolean;
  singbox_url_template: string;
  default_interval_minutes: number;
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

export function localOverrideRulesets(): Promise<RuleSetStatusView[]> {
  return invoke<RuleSetStatusView[]>("local_override_rulesets");
}

export function localOverrideToggleRuleset(communityId: string, subscribed: boolean): Promise<boolean> {
  return invoke<boolean>("local_override_toggle_ruleset", { communityId, subscribed });
}

export function localOverrideUpdateRulesetsNow(): Promise<number> {
  return invoke<number>("local_override_update_rulesets_now");
}
