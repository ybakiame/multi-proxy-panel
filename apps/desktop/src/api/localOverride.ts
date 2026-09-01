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
  mihomo_url_template: string;
  default_interval_minutes: number;
}

export interface AppliedTemplateView {
  template_id: string;
  applied_at: number;
  generated_rule_ids: string[];
}

export interface LocalOverrideView {
  singbox: CoreLocalOverrideView;
  mihomo: CoreLocalOverrideView;
  rule_set_subscriptions: RuleSetSubscriptionView[];
  applied_templates: AppliedTemplateView[];
}

export interface RuleSetStatusView {
  id: string;
  community_id: string;
  display_name: string;
  category: string;
  subscribed: boolean;
  singbox_cached: boolean;
  mihomo_cached: boolean;
  last_updated: number;
}

export interface SaveLocalOverrideInput {
  singbox: CoreLocalOverrideInput;
  mihomo: CoreLocalOverrideInput;
  rule_set_subscriptions: RuleSetSubscriptionInput[];
  applied_templates: AppliedTemplateInput[];
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
  mihomo_url_template: string;
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
