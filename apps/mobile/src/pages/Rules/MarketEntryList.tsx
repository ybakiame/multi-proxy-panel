import { CheckIcon, PlusIcon } from "@heroicons/react/24/outline";
import { Button, Card, Chip } from "@heroui/react";
import type { MetaCubeEntry } from "@pp/client-core";

interface MarketEntryListProps {
  entries: MetaCubeEntry[];
  /** 已添加规则集的 URL 集合（按 URL 匹配判定）。 */
  addedUrls: Set<string>;
  /** 正在添加的条目 id。 */
  addingKey: string | null;
  onAdd: (entry: MetaCubeEntry) => void;
}

/**
 * MetaCubeX 检索结果列表：条目卡（名称 + 「IP」/「Site」分组 chip + 一键添加）。
 */
export function MarketEntryList({ entries, addedUrls, addingKey, onAdd }: MarketEntryListProps) {
  return (
    <div className="flex flex-col gap-2">
      {entries.map((entry) => {
        const added = addedUrls.has(entry.url);
        const pending = addingKey === entry.id;
        return (
          <Card key={entry.id}>
            <Card.Content className="flex items-center justify-between gap-3 p-3">
              <div className="flex min-w-0 items-center gap-2">
                <span className="truncate text-sm font-medium text-foreground">{entry.name}</span>
                <Chip size="sm" variant="soft" color={entry.group === "ip" ? "accent" : "default"} className="shrink-0">
                  {entry.group === "ip" ? "IP" : "Site"}
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
                  onPress={() => onAdd(entry)}
                >
                  {!pending && <PlusIcon className="size-4" aria-hidden="true" />}
                  添加
                </Button>
              )}
            </Card.Content>
          </Card>
        );
      })}
    </div>
  );
}
