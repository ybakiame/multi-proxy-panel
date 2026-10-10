import { useQuery } from "@tanstack/react-query";
import { CONFIG_PREVIEW_KEY, previewCoreConfig, toErrorMessage } from "@pp/client-core";
import { InlineAlert } from "@pp/ui";
import { BottomSheet, Button } from "@pp/ui";

interface ConfigPreviewModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** 预览用的订阅 id（`null` = 按当前生效订阅合成）。 */
  subscriptionId?: string | null;
}

/**
 * 配置预览底部弹层（ADR-0003 M5）：打开时按指定订阅（或当前生效订阅）生成合成核心配置，
 * 以只读等宽字体滚动区展示。刻意不复制 desktop 的 ConfigPreviewModal（无编辑器/复制能力）。
 */
export function ConfigPreviewModal({ isOpen, onClose, subscriptionId }: ConfigPreviewModalProps) {
  const {
    data: content,
    error,
    isLoading,
  } = useQuery<string>({
    queryKey: [...CONFIG_PREVIEW_KEY, subscriptionId ?? null],
    queryFn: () => previewCoreConfig(subscriptionId),
    enabled: isOpen,
    retry: false,
  });

  return (
    <BottomSheet
      opened={isOpen}
      onClose={onClose}
      title="配置预览"
      footer={
        <Button variant="secondary" className="min-h-11 w-full" onPress={onClose}>
          关闭
        </Button>
      }
    >
      <div className="flex flex-col gap-4">
        {isLoading ? (
          <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
            <span className="text-sm text-zinc-500 dark:text-zinc-400">正在生成配置预览…</span>
            <span className="text-xs text-zinc-400 dark:text-zinc-500">拉取订阅节点并合成最终运行配置（只读）</span>
          </div>
        ) : error ? (
          <InlineAlert kind="danger" title="生成预览失败">
            <span className="break-all">{toErrorMessage(error)}</span>
          </InlineAlert>
        ) : (
          <pre className="max-h-[55vh] overflow-auto whitespace-pre-wrap break-words rounded-md border border-zinc-200 bg-zinc-50 p-3 font-mono text-xs leading-relaxed text-zinc-900 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100">
            {content}
          </pre>
        )}
      </div>
    </BottomSheet>
  );
}
