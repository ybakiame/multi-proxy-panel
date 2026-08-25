import { useEffect, useState } from "react";
import { Alert, Button, Modal } from "@heroui/react";
import { previewCoreConfig, toErrorMessage } from "../api";

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
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [content, setContent] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) {
      return;
    }
    let cancelled = false;
    setLoading(true);
    setError(null);
    setContent(null);
    void previewCoreConfig(subscriptionId)
      .then((text) => {
        if (!cancelled) {
          setContent(text);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(toErrorMessage(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen, subscriptionId]);

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
            {loading ? (
              <div className="flex flex-col items-center justify-center gap-2 py-10 text-center">
                <span className="text-sm text-muted">正在生成配置预览…</span>
                <span className="text-xs text-muted/80">拉取订阅节点并按当前核心合成最终配置（只读）</span>
              </div>
            ) : error ? (
              <Alert status="danger">
                <Alert.Indicator />
                <Alert.Content>
                  <Alert.Title>生成预览失败</Alert.Title>
                  <Alert.Description>{error}</Alert.Description>
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
