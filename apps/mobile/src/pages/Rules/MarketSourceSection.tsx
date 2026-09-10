import { useState } from "react";
import { ArrowPathIcon, PlusIcon, TrashIcon } from "@heroicons/react/24/outline";
import { AlertDialog, Button, Card, Chip } from "@heroui/react";
import type { MarketSourceView } from "@pp/client-core";

interface MarketSourceSectionProps {
  sources: MarketSourceView[];
  /** 添加市场源（打开表单 Sheet）。 */
  onAdd: () => void;
  /** 刷新单个源。 */
  onRefresh: (source: MarketSourceView) => void;
  /** 删除单个源（确认后调用）。 */
  onRemove: (source: MarketSourceView) => void;
  /** 正在刷新的源 id（禁用其刷新按钮并显示 pending）。 */
  refreshingId: string | null;
}

/** 上次拉取时间格式化：0 → 从未拉取。 */
function formatFetched(lastFetched: number): string {
  if (!lastFetched) return "从未拉取";
  return new Date(lastFetched * 1000).toLocaleString();
}

/**
 * 市场源管理区（顶部）：源列表卡（名称 / URL 截断 / 条目数 / 上次拉取时间）
 * + 「添加」按钮 + 每源「刷新」/「删除」（删除需 AlertDialog 确认）。
 */
export function MarketSourceSection({ sources, onAdd, onRefresh, onRemove, refreshingId }: MarketSourceSectionProps) {
  const [pendingDelete, setPendingDelete] = useState<MarketSourceView | null>(null);

  const handleDeleteConfirm = () => {
    const target = pendingDelete;
    setPendingDelete(null);
    if (target) onRemove(target);
  };

  return (
    <section className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <span className="min-w-0 truncate text-sm font-medium text-foreground">市场源</span>
        <Button variant="secondary" className="min-h-11 shrink-0 px-3" onPress={onAdd}>
          <PlusIcon className="size-4" aria-hidden="true" />
          添加
        </Button>
      </div>

      <div className="flex flex-col gap-2">
        {sources.map((source) => {
          const refreshing = refreshingId === source.id;
          return (
            <Card key={source.id}>
              <Card.Content className="flex flex-col gap-3 p-3">
                <div className="flex min-w-0 flex-col gap-0.5">
                  <div className="flex min-w-0 items-center gap-1.5">
                    <span className="truncate text-sm font-medium text-foreground">{source.name}</span>
                    <Chip size="sm" variant="soft" color={source.kind === "github_releases" ? "accent" : "default"}>
                      {source.kind === "github_releases" ? "GitHub" : "JSON"}
                    </Chip>
                  </div>
                  <span className="truncate font-mono text-xs text-muted" title={source.url}>
                    {source.url}
                  </span>
                  <span className="text-xs text-muted">
                    {source.entry_count} 个条目 · 上次拉取：{formatFetched(source.last_fetched)}
                  </span>
                </div>
                <div className="flex items-center justify-end gap-2">
                  <Button
                    variant="secondary"
                    className="min-h-11 shrink-0 px-3"
                    isDisabled={refreshingId !== null}
                    isPending={refreshing}
                    onPress={() => onRefresh(source)}
                  >
                    {!refreshing && <ArrowPathIcon className="size-4" aria-hidden="true" />}
                    刷新
                  </Button>
                  <Button variant="danger" className="min-h-11 shrink-0 px-3" onPress={() => setPendingDelete(source)}>
                    <TrashIcon className="size-4" aria-hidden="true" />
                    删除
                  </Button>
                </div>
              </Card.Content>
            </Card>
          );
        })}
      </div>

      <AlertDialog.Backdrop
        isOpen={pendingDelete !== null}
        onOpenChange={(open) => {
          if (!open) setPendingDelete(null);
        }}
      >
        <AlertDialog.Container size="sm">
          <AlertDialog.Dialog>
            <AlertDialog.CloseTrigger />
            <AlertDialog.Header>
              <AlertDialog.Icon status="danger" />
              <AlertDialog.Heading>删除市场源</AlertDialog.Heading>
            </AlertDialog.Header>
            <AlertDialog.Body>
              <p className="break-words">确定删除市场源「{pendingDelete?.name ?? ""}」吗？将同时清理其本地目录缓存。</p>
            </AlertDialog.Body>
            <AlertDialog.Footer>
              <Button slot="close" variant="tertiary" onPress={() => setPendingDelete(null)}>
                取消
              </Button>
              <Button slot="close" variant="danger" onPress={handleDeleteConfirm}>
                删除
              </Button>
            </AlertDialog.Footer>
          </AlertDialog.Dialog>
        </AlertDialog.Container>
      </AlertDialog.Backdrop>
    </section>
  );
}
