import { ConfirmDialog } from "@pp/ui";

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
    <ConfirmDialog opened={isOpen} title={title} danger confirmText="删除" onConfirm={onConfirm} onClose={onClose}>
      <p className="break-words">{description}</p>
    </ConfirmDialog>
  );
}
