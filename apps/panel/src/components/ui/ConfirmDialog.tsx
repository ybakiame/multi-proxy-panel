import { useTranslation } from "react-i18next";
import { Button, Modal } from "@heroui/react";

interface ConfirmDialogProps {
  title: string;
  children: React.ReactNode;
  isOpen: boolean;
  onClose: () => void;
  onConfirm: () => void;
  confirmText?: string;
  isLoading?: boolean;
}

export function ConfirmDialog({
  title,
  children,
  isOpen,
  onClose,
  onConfirm,
  confirmText,
  isLoading,
}: ConfirmDialogProps) {
  const { t } = useTranslation();
  return (
    <Modal.Backdrop
      isOpen={isOpen}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <Modal.Container>
        <Modal.Dialog>
          <Modal.Header>
            <Modal.Heading>{title}</Modal.Heading>
          </Modal.Header>
          <Modal.Body>{children}</Modal.Body>
          <Modal.Footer>
            <Button slot="close" variant="ghost" onPress={onClose}>
              {t("common.cancel")}
            </Button>
            <Button variant="danger" onPress={onConfirm} isPending={isLoading}>
              {confirmText || t("common.delete")}
            </Button>
          </Modal.Footer>
        </Modal.Dialog>
      </Modal.Container>
    </Modal.Backdrop>
  );
}
