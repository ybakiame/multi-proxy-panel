/**
 * 规则管理纯函数（Desktop / Mobile 双端共享，ADR-0003 M5.4）。
 *
 * 自 desktop Rules 页上移：标签映射、场景模板定义、摘要/详情行格式化与
 * View → Input 转换。不含任何 UI / 平台依赖，双端页面只消费本模块。
 */

import type {
  CoreLocalOverrideInput,
  CoreLocalOverrideView,
  LocalOverrideView,
  LocalRuleView,
} from "./api/localOverride";

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

export function buildSaveInput(view: LocalOverrideView, patchCore?: CoreLocalOverrideInput) {
  return {
    singbox: patchCore ?? viewToInput(view.singbox),
    applied_templates: view.applied_templates.map((t) => ({
      template_id: t.template_id,
      applied_at: t.applied_at,
      generated_rule_ids: t.generated_rule_ids,
    })),
    // 自定义规则集整段透传（View/Input 同构，去掉只读的 cached 字段；无 enabled）。
    custom_rule_sets: view.custom_rule_sets.map((rs) => ({
      id: rs.id,
      name: rs.name,
      tag: rs.tag,
      source: rs.source,
      last_updated: rs.last_updated,
    })),
    // 自定义场景模板整段透传（rules = 规则 ID 引用列表）。
    custom_templates: view.custom_templates.map((t) => ({
      id: t.id,
      name: t.name,
      desc: t.desc,
      rules: t.rules,
      created_at: t.created_at,
    })),
  };
}
