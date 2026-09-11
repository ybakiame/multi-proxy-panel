import { AlertDialog, Button } from "@heroui/react";

interface OutboundDeleteConfirmProps {
  isOpen: boolean;
  title: string;
  description: string;
  onClose: () => void;
  onConfirm: () => void;
}

/**
 * 自定义出站页删除确认框（ADR-0005 P0-4c，对齐 `DnsDeleteConfirm`）。
 *
 * 由编辑 Sheet 的删除入口触发（先收起 Sheet 再弹确认，避免双层遮罩叠放）。
 */
export function OutboundDeleteConfirm({ isOpen, title, description, onClose, onConfirm }: OutboundDeleteConfirmProps) {
  return (
    <AlertDialog.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <AlertDialog.Container size="sm">
        <AlertDialog.Dialog>
          <AlertDialog.CloseTrigger />
          <AlertDialog.Header>
            <AlertDialog.Icon status="danger" />
            <AlertDialog.Heading>{title}</AlertDialog.Heading>
          </AlertDialog.Header>
          <AlertDialog.Body>
            <p className="break-words">{description}</p>
          </AlertDialog.Body>
          <AlertDialog.Footer>
            <Button slot="close" variant="tertiary" onPress={onClose}>
              取消
            </Button>
            <Button slot="close" variant="danger" onPress={onConfirm}>
              删除
            </Button>
          </AlertDialog.Footer>
        </AlertDialog.Dialog>
      </AlertDialog.Container>
    </AlertDialog.Backdrop>
  );
}
