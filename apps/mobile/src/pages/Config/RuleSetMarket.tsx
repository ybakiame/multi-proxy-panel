import { useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { MagnifyingGlassIcon, SparklesIcon } from "@heroicons/react/24/outline";
import { Alert, Button, Card, Spinner } from "@heroui/react";
import {
  LOCAL_OVERRIDE_KEY,
  META_CUBE_SOURCE,
  buildSaveInput,
  localOverrideGet,
  localOverrideSave,
  searchMetaCubeEntries,
  toErrorMessage,
  toastError,
  toastSuccess,
} from "@pp/client-core";
import type { CustomRuleSetInput, LocalOverrideView, MetaCubeEntry } from "@pp/client-core";
import { BackHeader } from "../../components/BackHeader";
import { MarketEntryList } from "./MarketEntryList";
import { asArray, isLocalOverrideView } from "./localOverrideGuards";

/** 常用检索词快捷 Chip（点击填充检索框）。 */
const POPULAR_KEYWORDS = ["cn", "ads", "google", "netflix", "youtube", "telegram"];

/** 分类过滤分段控件选项：全部 / GeoIP（IP 段）/ GeoSite（域名）。 */
const CATEGORY_FILTERS = [
  { id: "all", label: "全部" },
  { id: "geoip", label: "GeoIP" },
  { id: "geosite", label: "GeoSite" },
] as const;

type CategoryFilter = (typeof CATEGORY_FILTERS)[number]["id"];

const inputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface pl-10 pr-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

/**
 * 规则集市场页（路由 `/config/route/rulesets/market`）。
 *
 * 市场为 **MetaCubeX/meta-rules-dat 固定源**（sing 分支 `geo/{geoip,geosite}/*.srs`）：
 * - 内置文件名清单静态硬编码于 `@pp/client-core` 的 `metaCubeCatalog`，检索在本地
 *   完成（大小写不敏感 `contains`），**不做运行时 GitHub API 调用**（规避 403 限流）；
 * - 一键添加构建 Remote 规则集（raw 直链 + `binary`），写入 `custom_rule_sets`
 *   后需在规则集管理页「立即更新」下载；已添加按 URL 匹配判定；
 * - 自定义需求走规则集管理页的「添加」表单（远程 URL）。
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

  // 结构守卫：缓存残留异构形态时视为未加载，渲染加载/空态而非崩溃。
  const overrideData = isLocalOverrideView(rawOverride) ? rawOverride : null;
  const customSets = asArray(overrideData?.custom_rule_sets);
  const invalidate = () => void queryClient.invalidateQueries({ queryKey: LOCAL_OVERRIDE_KEY });

  // ---- 局部 UI 状态 ----
  const [keyword, setKeyword] = useState("");
  const [category, setCategory] = useState<CategoryFilter>("all");
  const [addingKey, setAddingKey] = useState<string | null>(null);

  // 本地检索：空关键词返回空数组（渲染引导态）；分类过滤与文本检索叠加生效。
  const results = useMemo(
    () => searchMetaCubeEntries(keyword, category === "all" ? undefined : category),
    [keyword, category],
  );

  // 已添加判定：按 URL 匹配。
  const addedUrls = useMemo(
    () => new Set(customSets.flatMap((rs) => (rs.source.kind === "remote" ? [rs.source.url] : []))),
    [customSets],
  );

  // ---- 一键添加：custom 段整段替换追加（Remote + binary 声明格式） ----
  const handleAddEntry = async (entry: MetaCubeEntry) => {
    if (!overrideData || addingKey !== null || addedUrls.has(entry.url)) return;
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

  const hasKeyword = keyword.trim().length > 0;
  // 空结果文案前缀：选中分类时点明过滤范围（全部则留空）。
  const filterPrefix =
    category === "all" ? "" : `${CATEGORY_FILTERS.find((item) => item.id === category)?.label ?? ""} `;

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

        {overrideData && (
          <>
            {/* 固定源说明卡 */}
            <Card>
              <Card.Content className="flex flex-col gap-2 p-4">
                <div className="flex items-center gap-2">
                  <SparklesIcon className="size-5 shrink-0 text-accent" aria-hidden="true" />
                  <span className="min-w-0 truncate text-sm font-medium text-foreground">{META_CUBE_SOURCE.name}</span>
                </div>
                <span className="text-xs text-muted">{META_CUBE_SOURCE.description}</span>
                <span className="text-xs text-muted">更新频率：{META_CUBE_SOURCE.updateFrequency}</span>
              </Card.Content>
            </Card>

            {/* 检索框 */}
            <div className="relative">
              <MagnifyingGlassIcon
                className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted"
                aria-hidden="true"
              />
              <input
                aria-label="检索规则集"
                value={keyword}
                onChange={(event) => setKeyword(event.target.value)}
                placeholder="输入关键词检索规则集"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                className={inputClass}
              />
            </div>

            {/* 分类过滤：全部 / GeoIP / GeoSite，与文本检索叠加生效 */}
            <fieldset className="flex min-w-0 gap-1 rounded-xl border border-border/60 bg-surface-secondary/40 p-1">
              <legend className="sr-only">规则集分类过滤</legend>
              {CATEGORY_FILTERS.map((item) => (
                <Button
                  key={item.id}
                  variant={category === item.id ? "primary" : "secondary"}
                  size="sm"
                  className="min-h-11 flex-1"
                  onPress={() => setCategory(item.id)}
                >
                  {item.label}
                </Button>
              ))}
            </fieldset>

            {hasKeyword ? (
              results.length === 0 ? (
                <Card>
                  <Card.Content className="flex flex-col items-center justify-center px-6 py-8 text-center">
                    <span className="text-sm text-muted">
                      没有匹配「{keyword.trim()}」的{filterPrefix}规则集
                    </span>
                  </Card.Content>
                </Card>
              ) : (
                <>
                  <span className="text-xs text-muted">
                    共 {results.length} 个结果；添加后需在规则集管理页点击「立即更新」下载
                  </span>
                  <MarketEntryList
                    entries={results}
                    addedUrls={addedUrls}
                    addingKey={addingKey}
                    onAdd={(entry) => void handleAddEntry(entry)}
                  />
                </>
              )
            ) : (
              <>
                <span className="text-sm text-muted">输入关键词检索规则集（如 netflix / cn / google）</span>
                <section className="flex flex-col gap-2">
                  <span className="text-xs text-muted">常用关键词</span>
                  <div className="flex flex-wrap gap-2">
                    {POPULAR_KEYWORDS.map((item) => (
                      <button
                        key={item}
                        type="button"
                        onClick={() => setKeyword(item)}
                        className="min-h-11 shrink-0 rounded-full border border-border/60 bg-surface px-4 text-sm font-medium text-foreground transition-colors active:opacity-70"
                      >
                        {item}
                      </button>
                    ))}
                  </div>
                </section>
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
