import { Alert, Button, Modal } from "@heroui/react";
import { useQuery } from "@tanstack/react-query";
import { previewCoreConfig, toErrorMessage } from "@pp/client-core";
import { CONFIG_PREVIEW_KEY } from "@pp/client-core";

interface ConfigPreviewModalProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  subscriptionId?: string | null;
}

/**
 * 配置预览弹窗：打开时按指定订阅（或当前生效订阅）生成合成核心配置并以只读方式展示。
 * 打开（isOpen 变 true）或 subscriptionId 变化时重新拉取。
 */
export default function ConfigPreviewModal({ isOpen, onClose, title, subscriptionId }: ConfigPreviewModalProps) {
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
        if (!open) {
          onClose();
        }
      }}
    >
      <Modal.Container>
        <Modal.Dialog className="sm:max-w-[760px]">
          <Modal.CloseTrigger />
          <Modal.Header>
            <Modal.Heading>{title}</Modal.Heading>
          </Modal.Header>
          <Modal.Body className="flex flex-col gap-4">
            {isLoading ? (
              <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                <span className="text-sm text-muted">正在生成配置预览…</span>
                <span className="text-xs text-muted/80">拉取订阅节点并按当前核心合成最终配置（只读）</span>
              </div>
            ) : error ? (
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>生成预览失败</Alert.Title>
                  <Alert.Description>{toErrorMessage(error)}</Alert.Description>
                </Alert.Content>
              </Alert>
            ) : (
              <pre className="max-h-[60vh] overflow-auto whitespace-pre-wrap break-words rounded-md border border-border bg-surface-secondary/40 p-3 font-mono text-xs leading-relaxed text-foreground">
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
