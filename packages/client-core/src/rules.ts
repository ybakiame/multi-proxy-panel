/**
 * 规则管理纯函数（Desktop / Mobile 双端共享，ADR-0003 M5.4）。
 *
 * 自 desktop Rules 页上移：标签映射、摘要/详情行格式化与 View → Input
 * 转换。不含任何 UI / 平台依赖，双端页面只消费本模块。
 */

import type { BaselineRuleSetView, BaselineRuleView } from "./api/baseline";
import type {
  CoreLocalOverrideInput,
  CoreLocalOverrideView,
  CustomRuleSetInput,
  CustomRuleSetView,
  LocalOverrideView,
  LocalRuleInput,
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

/**
 * 解析 `rule_set` 规则的 `target` 为独立 tag 列表：英文逗号拆分、逐段 trim、
 * 丢弃空段、按首次出现顺序去重（对齐 Rust `parse_rule_set_tags`）。
 *
 * 存量单值 target（如 `geosite-cn`）返回单元素数组，行为与旧逻辑一致。
 */
export function parseRuleSetTags(target: string): string[] {
  const seen = new Set<string>();
  const tags: string[] = [];
  for (const segment of target.split(",")) {
    const tag = segment.trim();
    if (tag === "" || seen.has(tag)) continue;
    seen.add(tag);
    tags.push(tag);
  }
  return tags;
}

/** `rule_set` 目标展示：多 tag 以 ` + ` 连接（如 `a + b`），单 tag 原样返回。 */
export function formatRuleSetTarget(target: string): string {
  return parseRuleSetTags(target).join(" + ");
}

export function actionLabel(action: string): string {
  if (isOutboundAction(action)) return ACTION_LABELS.outbound;
  return ACTION_LABELS[action] ?? action;
}

export function ruleSummary(rule: LocalRuleView): string {
  if (rule.name.trim()) return rule.name;
  // rule_set 目标可能为逗号分隔多 tag，展示为 `a + b`；其余类型原样。
  const target = rule.match_type === "rule_set" ? formatRuleSetTarget(rule.target) : rule.target;
  const base = `${matchTypeLabel(rule.match_type)}: ${target}`;
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
  };
}

/**
 * 按基线还原**内置规则集**（「重置内置规则集」）。
 *
 * 内置规则集是普通条目（可编辑 / 删除），还原即按规格重建：缺失项追加到末尾，
 * 已存在项按 id 覆写 name / tag / URL（缓存时间戳 `last_updated` /
 * `remote_updated_at` 保留，避免无谓重下）。用户自建条目不受影响。
 */
export function restoreBuiltinRuleSets(
  view: LocalOverrideView,
  specs: readonly Pick<BaselineRuleSetView, "id" | "name" | "tag" | "url">[],
): CustomRuleSetInput[] {
  const next: CustomRuleSetInput[] = view.custom_rule_sets.map((rs) => ({
    id: rs.id,
    name: rs.name,
    tag: rs.tag,
    source: rs.source,
    last_updated: rs.last_updated,
    remote_updated_at: rs.remote_updated_at,
    builtin: rs.builtin === true,
  }));

  for (const spec of specs) {
    const rebuilt: CustomRuleSetInput = {
      id: spec.id,
      name: spec.name,
      tag: spec.tag,
      source: { kind: "remote", url: spec.url, format: "binary" },
      last_updated: 0,
      remote_updated_at: 0,
      builtin: true,
    };
    const index = next.findIndex((rs) => rs.id === spec.id);
    if (index >= 0) {
      const existing = next[index]!;
      next[index] = {
        ...rebuilt,
        // 已下载过的缓存不必重下：保留本地 / 远端时间戳。
        last_updated: existing.last_updated,
        remote_updated_at: existing.remote_updated_at ?? 0,
      };
    } else {
      next.push(rebuilt);
    }
  }
  return next;
}

/**
 * 按基线还原**内置规则**（「恢复内置规则」）。
 *
 * 与规则集同理：内置规则是普通规则（可编辑 / 删除）。还原时缺失项**置顶插入**
 * （`sort_order` 低于现有最小值），已存在项按 id 覆写名称 / 匹配 / 动作，
 * 保留用户的启停与排序位置。
 */
export function restoreBuiltinRules(
  view: CoreLocalOverrideView,
  specs: readonly Pick<BaselineRuleView, "id" | "name" | "rule_set_tags" | "outbound">[],
): LocalRuleInput[] {
  const next: LocalRuleInput[] = view.rules.map((r) => ({
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
    builtin: r.builtin,
  }));

  for (const spec of specs) {
    const target = spec.rule_set_tags.join(",");
    const index = next.findIndex((r) => r.id === spec.id);
    if (index >= 0) {
      next[index] = {
        ...next[index]!,
        name: spec.name,
        match_type: "rule_set",
        target,
        action: spec.outbound,
        builtin: true,
      };
      continue;
    }
    // 置顶：比当前最小 sort_order 还小（无规则时为 0）。
    const topSort = next.reduce((min, r) => Math.min(min, r.sort_order), 1) - 1;
    next.unshift({
      id: spec.id,
      name: spec.name,
      enabled: true,
      match_type: "rule_set",
      target,
      action: spec.outbound,
      no_resolve: false,
      invert: false,
      note: "内置规则：可修改、调整顺序或删除，支持一键还原",
      created_at: Math.floor(Date.now() / 1000),
      sort_order: topSort,
      builtin: true,
    });
  }
  return next;
}

/** 内置规则集是否齐全（用于「重置内置规则集」入口的显隐 / 提示）。 */
export function hasAllBuiltinRuleSets(
  sets: readonly CustomRuleSetView[],
  specs: readonly Pick<BaselineRuleSetView, "id">[],
): boolean {
  return specs.every((spec) => sets.some((rs) => rs.id === spec.id));
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
      // 内置条目透传标记：后端按 id 归一化，用户删除后不会被重新播种。
      builtin: rs.builtin === true,
    })),
  };
}
