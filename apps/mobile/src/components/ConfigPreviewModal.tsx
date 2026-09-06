import { useQuery } from "@tanstack/react-query";
import { Alert, Button, Modal } from "@heroui/react";
import { CONFIG_PREVIEW_KEY, previewCoreConfig, toErrorMessage } from "@pp/client-core";

interface ConfigPreviewModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** 预览用的订阅 id（`null` = 按当前生效订阅合成）。 */
  subscriptionId?: string | null;
}

/**
 * 配置预览弹窗（ADR-0003 M5）：打开时按指定订阅（或当前生效订阅）生成合成核心配置，
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
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      isDismissable
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[640px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>配置预览</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex flex-col gap-4">
            {isLoading ? (
              <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                <span className="text-sm text-muted">正在生成配置预览…</span>
                <span className="text-xs text-muted/80">拉取订阅节点并合成最终运行配置（只读）</span>
              </div>
            ) : error ? (
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>生成预览失败</Alert.Title>
                  <Alert.Description className="break-all">{toErrorMessage(error)}</Alert.Description>
                </Alert.Content>
              </Alert>
            ) : (
              <pre className="max-h-[55vh] overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-surface-secondary/40 p-3 font-mono text-xs leading-relaxed text-foreground">
                {content}
              </pre>
            )}
          </Modal.Body>
          <Modal.Footer>
            <Button slot="close" variant="secondary" onPress={onClose}>
              关闭
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
