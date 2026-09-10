import { useMemo, useState } from "react";
import { CheckIcon, PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card, Chip } from "@heroui/react";
import type { MarketEntryView } from "@pp/client-core";

/** 分类筛选中「全部」的哨兵值。 */
const ALL_CATEGORY = "全部";

/** 条目格式 chip 文案：binary = srs 二进制，source = json 源码。 */
function formatLabel(format: MarketEntryView["format"]): string {
  return format === "binary" ? "srs" : "json";
}

interface MarketEntryListProps {
  entries: MarketEntryView[];
  /** 已添加规则集的 URL 集合（按 URL 匹配判定）。 */
  addedUrls: Set<string>;
  /** 正在添加的条目 id（跨源可能重复，用 source_id+id 组合）。 */
  addingKey: string | null;
  onAdd: (entry: MarketEntryView) => void;
}

/**
 * 市场条目区：合并全部源条目，分类筛选 Chip + 条目卡（来源 chip + 一键添加）。
 */
export function MarketEntryList({ entries, addedUrls, addingKey, onAdd }: MarketEntryListProps) {
  const [category, setCategory] = useState(ALL_CATEGORY);

  const categories = useMemo(
    () => [ALL_CATEGORY, ...Array.from(new Set(entries.map((item) => item.category).filter(Boolean)))],
    [entries],
  );

  const visibleEntries = useMemo(
    () => (category === ALL_CATEGORY ? entries : entries.filter((item) => item.category === category)),
    [entries, category],
  );

  return (
    <>
      {/* 分类筛选：水平滚动 Chip */}
      <div className="-mx-1 flex items-center gap-2 overflow-x-auto px-1 pb-0.5">
        {categories.map((item) => {
          const active = item === category;
          return (
            <button
              key={item}
              type="button"
              onClick={() => setCategory(item)}
              aria-pressed={active}
              className={`min-h-11 shrink-0 rounded-full border px-4 text-sm font-medium transition-colors ${
                active
                  ? "border-primary/60 bg-primary/10 text-primary"
                  : "border-border/60 bg-surface text-foreground active:opacity-70"
              }`}
            >
              {item}
            </button>
          );
        })}
      </div>

      {visibleEntries.length === 0 ? (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center px-6 py-8 text-center">
            <span className="text-sm text-muted">当前分类下没有条目</span>
          </Card.Content>
        </Card>
      ) : (
        <div className="flex flex-col gap-2">
          {visibleEntries.map((item) => {
            const key = `${item.source_id}:${item.id}`;
            const added = addedUrls.has(item.url);
            const pending = addingKey === key;
            return (
              <Card key={key}>
                <Card.Content className="flex flex-col gap-3 p-3">
                  <div className="flex min-w-0 flex-col gap-0.5">
                    <span className="truncate text-sm font-medium text-foreground">{item.name}</span>
                    {item.description && <span className="text-xs text-muted">{item.description}</span>}
                  </div>
                  <div className="flex items-center justify-between gap-2">
                    <div className="flex min-w-0 items-center gap-1.5">
                      {item.category && (
                        <Chip size="sm" variant="soft" color="default" className="shrink-0">
                          {item.category}
                        </Chip>
                      )}
                      <Chip size="sm" variant="soft" color="accent" className="shrink-0">
                        {formatLabel(item.format)}
                      </Chip>
                      <Chip size="sm" variant="soft" color="default" className="min-w-0 shrink">
                        <span className="truncate">{item.source_name}</span>
                      </Chip>
                    </div>
                    {added ? (
                      <Button variant="tertiary" isDisabled className="min-h-11 shrink-0 px-3">
                        <CheckIcon className="size-4" aria-hidden="true" />
                        已添加
                      </Button>
                    ) : (
                      <Button
                        variant="primary"
                        className="min-h-11 shrink-0 px-3"
                        isDisabled={addingKey !== null}
                        isPending={pending}
                        onPress={() => onAdd(item)}
                      >
                        {!pending && <PlusIcon className="size-4" aria-hidden="true" />}
                        添加
                      </Button>
                    )}
                  </div>
                </Card.Content>
              </Card>
            );
          })}
        </div>
      )}
    </>
  );
}
