import type { LocalOverrideView } from "@pp/client-core";
import { asArray } from "./localOverrideGuards";

/**
 * 规则集选择器选项（`rule_set` 匹配目标）：规则集 tag（社区 remote / 自定义 manual；
 * 规则集是纯资源无启停）。
 *
 * 同时供路由规则（`RuleEditSheet`）与 DNS 分流规则（`DnsRuleFormSheet`）复用，
 * 禁止各页另造数据源。
 */
export interface RuleSetOption {
  /** 写入 `target` 的原始值：规则集 tag。 */
  value: string;
  /** 下拉显示名（规则集 tag）。 */
  label: string;
  /** 可选说明行（友好名 / 内置来源 / 不可用原值的提示）。 */
  hint?: string;
}

/**
 * 构建规则集选择器候选。
 *
 * 可引用集合 = **全部自定义规则集**（`custom_rule_sets`：社区 remote / 手动 manual，
 * 以及已物化为普通条目的内置规则集）∪ **启用中的存量规则集引用**
 * （`singbox.rule_sets`，旧版遗留机制，注入时 enabled 即注册进 `route.rule_set`）。
 *
 * 规则集是纯资源、无启停概念，是否被注入只由引用它的规则决定；去重时以自定义
 * 规则集优先（其友好名更贴近用户命名）。
 */
export function buildRuleSetOptions(overrideData: LocalOverrideView | null | undefined): RuleSetOption[] {
  if (!overrideData) return [];
  const options: RuleSetOption[] = [];
  const seen = new Set<string>();
  const push = (tag: string, name: string, fallbackHint?: string) => {
    const value = tag.trim();
    if (value === "" || seen.has(value)) return;
    seen.add(value);
    options.push({ value, label: value, hint: name.trim() || fallbackHint });
  };

  // 自定义规则集（remote / manual）优先，纯资源无启停。
  for (const ruleSet of asArray(overrideData.custom_rule_sets)) {
    push(ruleSet.tag, ruleSet.name);
  }
  // 内置规则集引用（remote / bundled / local）：仅启用项会被注入，故仅列启用项。
  for (const ruleSet of asArray(overrideData.singbox.rule_sets)) {
    if (!ruleSet.enabled) continue;
    push(ruleSet.tag, ruleSet.name, "内置规则集");
  }

  return options;
}
