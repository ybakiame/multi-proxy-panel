/**
 * 规则管理纯函数（Desktop / Mobile 双端共享，ADR-0003 M5.4）。
 *
 * 自 desktop Rules 页上移：标签映射、摘要/详情行格式化与 View → Input
 * 转换。不含任何 UI / 平台依赖，双端页面只消费本模块。
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
  outbound: "指定出站",
};

export const RULE_ACTIONS = [
  { id: "proxy", label: "代理" },
  { id: "direct", label: "直连" },
  { id: "reject", label: "拒绝" },
  { id: "outbound", label: "指定出站" },
];

/**
 * `RuleAction::Outbound { tag }` 的 wire 前缀（对齐 Rust
 * `pp-client-tauri` 的 `views.rs::rule_action_str` / `convert.rs::parse_action`）。
 *
 * 数据变体序列化为 `"outbound:<tag>"`（非枚举 serde JSON 形态）；规则卡片把该字符串
 * 原样回写 `LocalRuleInput.action`，由后端解析回 `RuleAction::Outbound`。
 */
export const OUTBOUND_ACTION_PREFIX = "outbound:";

/** 是否为「指定出站」动作（`outbound:<tag>`）。 */
export function isOutboundAction(action: string): boolean {
  return action.startsWith(OUTBOUND_ACTION_PREFIX);
}

/** 从动作字符串提取出站 tag（非 outbound 动作返回空串）。 */
export function outboundTagFromAction(action: string): string {
  return isOutboundAction(action) ? action.slice(OUTBOUND_ACTION_PREFIX.length) : "";
}

/** 由出站 tag 构造 wire 动作字符串（tag 已 trim；空 tag 仍生成前缀，由表单校验拦截）。 */
export function buildOutboundAction(tag: string): string {
  return `${OUTBOUND_ACTION_PREFIX}${tag.trim()}`;
}

export function matchTypeLabel(type: string): string {
  return MATCH_TYPE_LABELS[type] ?? type;
}

export function actionLabel(action: string): string {
  if (isOutboundAction(action)) return ACTION_LABELS.outbound;
  return ACTION_LABELS[action] ?? action;
}

export function ruleSummary(rule: LocalRuleView): string {
  if (rule.name.trim()) return rule.name;
  const base = `${matchTypeLabel(rule.match_type)}: ${rule.target}`;
  const tag = isOutboundAction(rule.action) ? outboundTagFromAction(rule.action) : "";
  return tag ? `${base} → ${ACTION_LABELS.outbound}: ${tag}` : base;
}

export function ruleDetailLine(rule: LocalRuleView): string {
  const parts: string[] = [`→ ${actionLabel(rule.action)}`];
  const tag = isOutboundAction(rule.action) ? outboundTagFromAction(rule.action) : "";
  if (tag) parts.push(`(${tag})`);
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
    // 自定义规则集整段透传（View/Input 同构，去掉只读的 cached 字段；无 enabled）。
    custom_rule_sets: view.custom_rule_sets.map((rs) => ({
      id: rs.id,
      name: rs.name,
      tag: rs.tag,
      source: rs.source,
      last_updated: rs.last_updated,
      // 后端对已存在 id 会按磁盘现值回填；新条目（市场一键添加）据此携带远端时间。
      remote_updated_at: rs.remote_updated_at,
    })),
  };
}
