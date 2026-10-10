import { MagnifyingGlassIcon, SparklesIcon } from "@heroicons/react/24/outline";
import { InlineAlert } from "@pp/ui";
import { Button, Card, Spinner, inputClassName } from "@pp/ui";
import { META_CUBE_SOURCE, useRuleSetMarket } from "@pp/client-core";
import type { MarketCategoryFilter } from "@pp/client-core";
import { SubPageShell } from "../../components/PageShell";
import { MarketEntryList } from "./MarketEntryList";

/** 常用检索词快捷 Chip（点击填充检索框）。 */
const POPULAR_KEYWORDS = ["cn", "ads", "google", "netflix", "youtube", "telegram"];

/** 分类过滤分段控件选项：全部 / GeoIP（IP 段）/ GeoSite（域名）。 */
const CATEGORY_FILTERS = [
  { id: "all", label: "全部" },
  { id: "geoip", label: "GeoIP" },
  { id: "geosite", label: "GeoSite" },
] as const;

const inputClass = `${inputClassName} pl-10`;

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
  // 本地检索 / 添加 / 落盘逻辑已单源化至 client-core `useRuleSetMarket`（ADR-0011）。
  const market = useRuleSetMarket();
  const { keyword, setKeyword, category, setCategory, results, addingKey, addedUrls } = market;

  const hasKeyword = market.hasKeyword;
  // 空结果文案前缀：选中分类时点明过滤范围（全部则留空）。
  const filterPrefix =
    category === "all" ? "" : `${CATEGORY_FILTERS.find((item) => item.id === category)?.label ?? ""} `;

  return (
    <SubPageShell title="规则集市场">
      {market.isLoading && !market.ready && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-3 py-12 text-center">
            <Spinner aria-hidden="true" />
            <span className="text-sm text-zinc-500 dark:text-zinc-400">正在加载市场…</span>
          </Card.Content>
        </Card>
      )}

      {!market.ready && !market.isLoading && market.error && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-2 py-8 text-center">
            <InlineAlert kind="danger" title="加载失败">
              {market.error}
            </InlineAlert>
          </Card.Content>
        </Card>
      )}

      {!market.ready && !market.isLoading && !market.error && (
        <Card>
          <Card.Content className="flex flex-col items-center gap-3 py-12 text-center">
            <span className="text-sm text-zinc-500 dark:text-zinc-400">规则集数据不可用</span>
            <Button variant="secondary" className="min-h-11 shrink-0 px-4" onPress={() => market.invalidate()}>
              重新加载
            </Button>
          </Card.Content>
        </Card>
      )}

      {market.ready && (
        <>
          {/* 固定源说明卡 */}
          <Card>
            <Card.Content className="flex flex-col gap-2 p-4">
              <div className="flex items-center gap-2">
                <SparklesIcon className="size-5 shrink-0 text-primary" aria-hidden="true" />
                <span className="min-w-0 truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
                  {META_CUBE_SOURCE.name}
                </span>
              </div>
              <span className="text-xs text-zinc-500 dark:text-zinc-400">{META_CUBE_SOURCE.description}</span>
              <span className="text-xs text-zinc-500 dark:text-zinc-400">
                更新频率：{META_CUBE_SOURCE.updateFrequency}
              </span>
            </Card.Content>
          </Card>

          {/* 检索框 */}
          <div className="relative">
            <MagnifyingGlassIcon
              className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-zinc-500 dark:text-zinc-400"
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
          <fieldset className="flex min-w-0 gap-1 rounded-xl border border-zinc-200 dark:border-zinc-700 bg-zinc-100 dark:bg-zinc-800 p-1">
            <legend className="sr-only">规则集分类过滤</legend>
            {CATEGORY_FILTERS.map((item) => (
              <Button
                key={item.id}
                variant={category === item.id ? "primary" : "secondary"}
                size="sm"
                className="min-h-11 flex-1"
                onPress={() => setCategory(item.id as MarketCategoryFilter)}
              >
                {item.label}
              </Button>
            ))}
          </fieldset>

          {hasKeyword ? (
            results.length === 0 ? (
              <Card>
                <Card.Content className="flex flex-col items-center justify-center px-6 py-8 text-center">
                  <span className="text-sm text-zinc-500 dark:text-zinc-400">
                    没有匹配「{keyword.trim()}」的{filterPrefix}规则集
                  </span>
                </Card.Content>
              </Card>
            ) : (
              <>
                <span className="text-xs text-zinc-500 dark:text-zinc-400">
                  共 {results.length} 个结果；添加后需在规则集管理页点击「立即更新」下载
                </span>
                <MarketEntryList
                  entries={results}
                  addedUrls={addedUrls}
                  addingKey={addingKey}
                  onAdd={(entry) => void market.addEntry(entry)}
                />
              </>
            )
          ) : (
            <>
              <span className="text-sm text-zinc-500 dark:text-zinc-400">
                输入关键词检索规则集（如 netflix / cn / google）
              </span>
              <section className="flex flex-col gap-2">
                <span className="text-xs text-zinc-500 dark:text-zinc-400">常用关键词</span>
                <div className="flex flex-wrap gap-2">
                  {POPULAR_KEYWORDS.map((item) => (
                    <button
                      key={item}
                      type="button"
                      onClick={() => setKeyword(item)}
                      className="min-h-11 shrink-0 rounded-full border border-zinc-200 dark:border-zinc-700 bg-white dark:bg-zinc-900 px-4 text-sm font-medium text-zinc-900 dark:text-zinc-100 transition-colors active:opacity-70"
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
    </SubPageShell>
  );
}
