/**
 * 规则集市场共享视图模型（ADR-0011）：MetaCubeX meta-rules-dat 固定源检索 +
 * 一键添加为 Remote 规则集。
 *
 * 市场检索为纯本地行为（`metaCubeCatalog` 静态清单，无运行时 GitHub API 调用）；
 * 添加 = 向 `custom_rule_sets` 追加 Remote 条目（raw 直链 + `binary` 格式），
 * 落盘走共享的 `localOverrideSave` 链路，下载由规则集管理页「立即更新」完成。
 * 桌面端（Market 页）与移动端（RuleSetMarket 页）共用本 hook，UI 库零耦合。
 */

import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { localOverrideGet, localOverrideSave, toErrorMessage } from "../api";
import type { CustomRuleSetInput, LocalOverrideView } from "../api";
import { buildSaveInput } from "../rules";
import { LOCAL_OVERRIDE_KEY } from "../api/keys";
import { asArray, isLocalOverrideView } from "../localOverrideGuards";
import { searchMetaCubeEntries } from "../metaCubeCatalog";
import type { MetaCubeCategory, MetaCubeEntry } from "../metaCubeCatalog";
import { toastError, toastSuccess } from "../toast";

/** 市场分类过滤：`all` 表示不过滤（hook 层归一化为 `undefined` 传给检索）。 */
export type MarketCategoryFilter = "all" | MetaCubeCategory;

export interface RuleSetMarketView {
  isLoading: boolean;
  error: string | null;
  /** 规则集数据是否可用（结构守卫通过）。 */
  ready: boolean;
  keyword: string;
  setKeyword: (value: string) => void;
  category: MarketCategoryFilter;
  setCategory: (value: MarketCategoryFilter) => void;
  /** 当前检索结果（空关键词为空数组，渲染引导态）。 */
  results: MetaCubeEntry[];
  /** 是否已有检索关键词。 */
  hasKeyword: boolean;
  /** 已添加条目的 URL 集合（按 URL 匹配）。 */
  addedUrls: Set<string>;
  /** 某条目是否已添加（按 URL 匹配）。 */
  isAdded: (entry: MetaCubeEntry) => boolean;
  /** 正在添加中的条目 id（并发互斥，同时只允许一条添加）。 */
  addingKey: string | null;
  addEntry: (entry: MetaCubeEntry) => Promise<void>;
  invalidate: () => void;
}

export function useRuleSetMarket(): RuleSetMarketView {
  const queryClient = useQueryClient();
  const {
    data: rawOverride,
    isLoading,
    error: queryError,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });

  // 结构守卫：缓存残留异构形态时视为未加载，渲染加载/空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const customSets = asArray(overrideData?.custom_rule_sets);
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  const [keyword, setKeyword] = useState("");
  const [category, setCategory] = useState<MarketCategoryFilter>("all");
  const [addingKey, setAddingKey] = useState<string | null>(null);

  const results = useMemo(
    () => searchMetaCubeEntries(keyword, category === "all" ? undefined : category),
    [keyword, category],
  );

  // 已添加判定：按 URL 匹配。
  const addedUrls = useMemo(
    () => new Set(customSets.flatMap((rs) => (rs.source.kind === "remote" ? [rs.source.url] : []))),
    [customSets],
  );

  const addEntry = async (entry: MetaCubeEntry) => {
    if (!overrideData || addingKey !== null || addedUrls.has(entry.url)) {
      return;
    }
    setAddingKey(entry.id);
    const input: CustomRuleSetInput = {
      id: crypto.randomUUID(),
      name: entry.name,
      tag: entry.id,
      source: { kind: "remote", url: entry.url, format: entry.format },
      // 新条目尚未下载，last_updated 归零，待「立即更新」。
      last_updated: 0,
      remote_updated_at: 0,
    };
    const base = buildSaveInput(overrideData);
    try {
      await localOverrideSave({ ...base, custom_rule_sets: [...base.custom_rule_sets, input] });
      toastSuccess(`已添加「${entry.name}」，点击立即更新下载`);
      invalidate();
    } catch (err) {
      // 保存失败（如 tag 冲突）保留现场，仅报错。
      toastError(toErrorMessage(err));
      invalidate();
    } finally {
      setAddingKey(null);
    }
  };

  return {
    isLoading,
    error: queryError ? toErrorMessage(queryError) : null,
    ready: overrideData !== null,
    keyword,
    setKeyword,
    category,
    setCategory,
    results,
    hasKeyword: keyword.trim().length > 0,
    addedUrls,
    isAdded: (entry) => addedUrls.has(entry.url),
    addingKey,
    addEntry,
    invalidate,
  };
}
