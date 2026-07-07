import { useTranslation } from "react-i18next";
import {
  Button,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
} from "@heroui/react";

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
    <Modal isOpen={isOpen} onClose={onClose}>
      <ModalContent>
        <ModalHeader>{title}</ModalHeader>
        <ModalBody>{children}</ModalBody>
        <ModalFooter>
          <Button variant="flat" onPress={onClose}>
            {t("common.cancel")}
          </Button>
          <Button color="danger" onPress={onConfirm} isLoading={isLoading}>
            {confirmText || t("common.delete")}
          </Button>
        </ModalFooter>
      </ModalContent>
    </Modal>
  );
}
