import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { SparklesIcon } from "@heroicons/react/24/outline";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  MARKET_ENTRIES_KEY,
  RECOMMENDED_MARKET_SOURCES,
  buildSaveInput,
  localOverrideGet,
  localOverrideMarketAdd,
  localOverrideMarketEntries,
  localOverrideMarketRefresh,
  localOverrideMarketRemove,
  localOverrideSave,
  toErrorMessage,
  toastError,
  toastSuccess,
} from "@pp/client-core";
import type { CustomRuleSetInput, LocalOverrideView, MarketEntryView, MarketSourceView } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { MarketEntryList } from "./MarketEntryList";
import { MarketSourceFormSheet } from "./MarketSourceFormSheet";
import { MarketSourceSection } from "./MarketSourceSection";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";

/** 目录 JSON 数组格式示例（引导空态折叠展示）。 */
const CATALOG_FORMAT_EXAMPLE = `[
  {
    "id": "ads",
    "name": "广告拦截",
    "description": "常见广告与追踪域名",
    "category": "广告",
    "format": "binary",
    "url": "https://example.com/ads.srs"
  }
]`;

/**
 * 规则集市场页（路由 `/rules/rulesets/market`）。
 *
 * 内置静态目录已取消，市场内容全部来自**用户添加的远程 JSON 目录源**：
 * - 无源空态：引导卡 + 「添加市场源」按钮 + 目录格式示例（折叠）；
 * - 源管理区：源列表（名称 / URL / 条目数 / 上次拉取）+ 添加 / 刷新 / 删除；
 * - 条目区：合并全部源缓存条目（带来源 chip）+ 分类筛选 + 一键添加
 *   （写入 `custom_rule_sets`，已添加按 URL 匹配）；
 * - 数据源：`local_override_market_entries`（纯读缓存，不拉网络）；「刷新源」
 *   单独触发网络拉取。
 */
export default function RuleSetMarket() {
  const queryClient = useQueryClient();
  const {
    data: rawOverride,
    isLoading,
    error,
  } = useQuery<LocalOverrideView>({
    queryKey: LOCAL_OVERRIDE_KEY,
    queryFn: localOverrideGet,
  });
  const { data: rawEntries } = useQuery<MarketEntryView[]>({
    queryKey: MARKET_ENTRIES_KEY,
    queryFn: localOverrideMarketEntries,
  });

  // 结构守卫：缓存残留异构形态时视为未加载，渲染加载/空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const customSets = asArray(overrideData?.custom_rule_sets);
  const sources = asArray(overrideData?.market_sources);
  const entries = asArray(rawEntries);
  const invalidate = () => {
    void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });
    void queryClient.invalidateQueries({ queryKey: MARKET_ENTRIES_KEY });
  };

  // ---- 局部 UI 状态 ----
  const [formOpen, setFormOpen] = useState(false);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);
  const [addingKey, setAddingKey] = useState<string | null>(null);
  const [addingRecommended, setAddingRecommended] = useState<string | null>(null);

  // 已添加判定：按 URL 匹配（同 tag 不同 URL 由 Rust 校验报冲突）。
  const addedUrls = useMemo(
    () => new Set(customSets.flatMap((rs) => (rs.source.kind === "remote" ? [rs.source.url] : []))),
    [customSets],
  );

  // ---- 源管理 ----
  const handleAddSource = async (name: string, url: string, tag?: string): Promise<boolean> => {
    try {
      const created = await localOverrideMarketAdd(name, url, tag);
      toastSuccess(`已添加市场源「${created.name}」（${created.entry_count} 个条目）`);
      invalidate();
      return true;
    } catch (err) {
      toastError(toErrorMessage(err));
      invalidate();
      return false;
    }
  };

  /** 推荐源「一键添加」：单输入框 + 自动识别（GitHub releases）。 */
  const handleAddRecommended = async (ownerRepo: string, tag: string, name: string): Promise<void> => {
    if (addingRecommended !== null) return;
    setAddingRecommended(ownerRepo);
    try {
      await handleAddSource(name, ownerRepo, tag);
    } finally {
      setAddingRecommended(null);
    }
  };

  const handleRefreshSource = async (source: MarketSourceView) => {
    if (refreshingId !== null) return;
    setRefreshingId(source.id);
    try {
      const count = await localOverrideMarketRefresh(source.id);
      toastSuccess(`已刷新「${source.name}」，共 ${count} 个条目`);
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      setRefreshingId(null);
      invalidate();
    }
  };

  const handleRemoveSource = async (source: MarketSourceView) => {
    try {
      await localOverrideMarketRemove(source.id);
      toastSuccess(`已删除市场源「${source.name}」`);
    } catch (err) {
      toastError(toErrorMessage(err));
    } finally {
      invalidate();
    }
  };

  // ---- 一键添加条目：custom 段整段替换追加（Remote + 声明格式） ----
  const handleAddEntry = async (entry: MarketEntryView) => {
    if (!overrideData || addingKey !== null || addedUrls.has(entry.url)) return;
    setAddingKey(`${entry.source_id}:${entry.id}`);
    const input: CustomRuleSetInput = {
      id: crypto.randomUUID(),
      name: entry.name,
      tag: entry.id,
      source: { kind: "remote", url: entry.url, format: entry.format },
      // 新条目尚未下载，last_updated 归零，待「立即更新」。
      last_updated: 0,
      // GitHub release asset 的 updated_at（JSON 目录可为 0）：写入远端更新时间，
      // 供「有更新」判定使用，比 HEAD 更准。
      remote_updated_at: entry.updated_at,
    };
    const base = buildSaveInput(overrideData);
    try {
      await localOverrideSave({ ...base, custom_rule_sets: [...base.custom_rule_sets, input] });
      toastSuccess("已添加，点击立即更新下载");
      invalidate();
    } catch (err) {
      // 保存失败（如 tag 冲突）保留现场，仅报错。
      toastError(toErrorMessage(err));
      invalidate();
    } finally {
      setAddingKey(null);
    }
  };

  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title="规则集市场" />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {isLoading && !overrideData && (
          <Card>
            <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
              <Spinner aria-hidden="true" />
              <span className="text-sm text-muted">正在加载市场…</span>
            </Card.Content>
          </Card>
        )}

        {!overrideData && !isLoading && error && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>加载失败</Alert.Title>
                  <Alert.Description>{toErrorMessage(error)}</Alert.Description>
                </Alert.Content>
              </Alert>
            </Card.Content>
          </Card>
        )}

        {!overrideData && !isLoading && !error && (
          <Card>
            <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
              <span className="text-sm text-muted">规则集数据不可用</span>
              <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => void invalidate()}>
                重新加载
              </Button>
            </Card.Content>
          </Card>
        )}

        {overrideData && sources.length === 0 && (
          <>
            <Card>
              <Card.Content className="flex flex-col items-center gap-4 px-6 py-10 text-center">
                <SparklesIcon className="size-10 text-muted" aria-hidden="true" />
                <div className="flex flex-col gap-1">
                  <span className="text-sm font-medium text-foreground">市场目录为空</span>
                  <span className="text-sm text-muted">添加一个规则集市场源</span>
                </div>
                <Button variant="primary" className="min-h-11 shrink-0 px-4" onPress={() => setFormOpen(true)}>
                  添加市场源
                </Button>
                <details className="w-full text-left">
                  <summary className="cursor-pointer text-xs text-muted">查看目录 JSON 格式示例</summary>
                  <pre className="mt-2 overflow-x-auto rounded-lg border border-border/60 bg-surface-secondary/60 p-3 text-left font-mono text-xs leading-5 text-foreground">
                    {CATALOG_FORMAT_EXAMPLE}
                  </pre>
                </details>
              </Card.Content>
            </Card>

            <section className="flex flex-col gap-2">
              <span className="text-sm font-medium text-foreground">推荐市场源</span>
              {RECOMMENDED_MARKET_SOURCES.map((rec) => {
                const pending = addingRecommended === rec.ownerRepo;
                return (
                  <Card key={rec.ownerRepo}>
                    <Card.Content className="flex flex-col gap-3 p-3">
                      <div className="flex min-w-0 flex-col gap-0.5">
                        <span className="truncate text-sm font-medium text-foreground">{rec.name}</span>
                        <span className="text-xs text-muted">{rec.description}</span>
                        <span className="truncate font-mono text-xs text-muted">
                          {rec.ownerRepo}
                          {rec.tag ? ` @ ${rec.tag}` : " @ latest"}
                        </span>
                      </div>
                      <Button
                        variant="primary"
                        className="min-h-11 shrink-0 self-end px-3"
                        isDisabled={addingRecommended !== null}
                        isPending={pending}
                        onPress={() => void handleAddRecommended(rec.ownerRepo, rec.tag, rec.name)}
                      >
                        一键添加
                      </Button>
                    </Card.Content>
                  </Card>
                );
              })}
            </section>
          </>
        )}

        {overrideData && sources.length > 0 && (
          <>
            <span className="text-xs text-muted">
              条目来自已添加市场源的本地缓存；「刷新」会重新拉取目录（走 GitHub 代理设置），
              添加后需在规则集管理页点击「立即更新」下载
            </span>

            <MarketSourceSection
              sources={sources}
              refreshingId={refreshingId}
              onAdd={() => setFormOpen(true)}
              onRefresh={(source) => void handleRefreshSource(source)}
              onRemove={(source) => void handleRemoveSource(source)}
            />

            {entries.length === 0 ? (
              <Card>
                <Card.Content className="flex flex-col items-center justify-center px-6 py-8 text-center">
                  <span className="text-sm text-muted">暂无缓存条目，点击上方「刷新」拉取目录</span>
                </Card.Content>
              </Card>
            ) : (
              <MarketEntryList
                entries={entries}
                addedUrls={addedUrls}
                addingKey={addingKey}
                onAdd={(entry) => void handleAddEntry(entry)}
              />
            )}
          </>
        )}
      </div>

      <MarketSourceFormSheet isOpen={formOpen} onClose={() => setFormOpen(false)} onSave={handleAddSource} />
    </div>
  );
}
