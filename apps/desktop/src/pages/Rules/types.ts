import type { CoreLocalOverrideInput, CoreLocalOverrideView, LocalOverrideView, LocalRuleView } from "../../api";

export const MATCH_TYPE_LABELS: Record<string, string> = {
  domain: "域名",
  domain_suffix: "域名后缀",
  domain_keyword: "域名关键词",
  ip_cidr: "IP 段",
  source_ip_cidr: "源 IP 段",
  rule_set: "规则集",
  app_package: "应用包名",
  process_name: "进程名",
  port: "端口",
  final: "最终规则",
};

export const ACTION_LABELS: Record<string, string> = {
  proxy: "代理",
  direct: "直连",
  reject: "拒绝",
};

export const TEMPLATE_DEFS: { id: string; name: string; desc: string }[] = [
  { id: "return_to_china", name: "回国模式", desc: "海外用户访问国内服务直连" },
  { id: "overseas", name: "海外模式", desc: "国内用户访问海外服务代理" },
  { id: "ad_filter", name: "广告过滤", desc: "拦截常见广告域名" },
];

export const RULE_ACTIONS = [
  { id: "proxy", label: "代理" },
  { id: "direct", label: "直连" },
  { id: "reject", label: "拒绝" },
];

export function matchTypeLabel(type: string): string {
  return MATCH_TYPE_LABELS[type] ?? type;
}

export function actionLabel(action: string): string {
  return ACTION_LABELS[action] ?? action;
}

export function ruleSummary(rule: LocalRuleView): string {
  if (rule.name.trim()) return rule.name;
  return `${matchTypeLabel(rule.match_type)}: ${rule.target}`;
}

export function ruleDetailLine(rule: LocalRuleView): string {
  const parts: string[] = [`→ ${actionLabel(rule.action)}`];
  if (rule.no_resolve) parts.push("[no-resolve]");
  if (rule.invert) parts.push("[invert]");
  return parts.join(" ");
}

export function viewToInput(view: CoreLocalOverrideView): CoreLocalOverrideInput {
  return {
    rules: view.rules.map((r) => ({
      id: r.id,
      name: r.name,
      enabled: r.enabled,
      match_type: r.match_type,
      target: r.target,
      action: r.action,
      no_resolve: r.no_resolve,
      invert: r.invert,
      note: r.note,
      created_at: r.created_at,
      sort_order: r.sort_order,
    })),
    rule_sets: view.rule_sets.map((rs) => ({
      id: rs.id,
      name: rs.name,
      tag: rs.tag,
      kind: rs.kind,
      source: rs.source,
      enabled: rs.enabled,
      auto_update_interval_minutes: rs.auto_update_interval_minutes,
      last_updated: rs.last_updated,
    })),
    enabled: view.enabled,
  };
}

export function buildSaveInput(
  view: LocalOverrideView,
  patchCore?: { key: "singbox" | "mihomo"; value: CoreLocalOverrideInput },
) {
  return {
    singbox: patchCore?.key === "singbox" ? patchCore.value : viewToInput(view.singbox),
    mihomo: patchCore?.key === "mihomo" ? patchCore.value : viewToInput(view.mihomo),
    rule_set_subscriptions: view.rule_set_subscriptions.map((s) => ({
      id: s.id,
      community_id: s.community_id,
      display_name: s.display_name,
      category: s.category,
      subscribed: s.subscribed,
      singbox_url_template: s.singbox_url_template,
      mihomo_url_template: s.mihomo_url_template,
      default_interval_minutes: s.default_interval_minutes,
    })),
    applied_templates: view.applied_templates.map((t) => ({
      template_id: t.template_id,
      applied_at: t.applied_at,
      generated_rule_ids: t.generated_rule_ids,
    })),
  };
}
