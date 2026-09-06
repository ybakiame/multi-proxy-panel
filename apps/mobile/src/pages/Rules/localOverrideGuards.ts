import type { LocalOverrideView } from "@pp/client-core";

/**
 * `LocalOverrideView` 结构守卫。
 *
 * 规则三页的查询共用 `LOCAL_OVERRIDE_KEY`，缓存只应存放 `local_override_get`
 * 返回的 canonical 形态。历史实现曾让规则集管理页把复合形态
 * `{ override, ruleSets }` 写入同一 key，返回规则主页时主页对 undefined 字段
 * （如 `applied_templates.map`）直接访问而整页崩溃（黑屏）。本守卫把非预期
 * 形态一律视为未加载，由页面渲染加载/空态，作为查询键隔离之后的第二道防线。
 */
export function isLocalOverrideView(value: LocalOverrideView | null | undefined): value is LocalOverrideView {
  if (!value || typeof value.singbox !== "object" || value.singbox === null) {
    return false;
  }
  return (
    Array.isArray(value.singbox.rules) &&
    Array.isArray(value.singbox.rule_sets) &&
    Array.isArray(value.rule_set_subscriptions) &&
    Array.isArray(value.applied_templates) &&
    Array.isArray(value.custom_rule_sets) &&
    Array.isArray(value.custom_templates)
  );
}

/** 数组形态守卫：独立查询（如规则集订阅状态）返回非数组时视为空。 */
export function asArray<T>(value: T[] | null | undefined): T[] {
  return Array.isArray(value) ? value : [];
}
