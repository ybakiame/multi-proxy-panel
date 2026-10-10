import { ConfirmDialog } from "@pp/ui";

interface DnsDeleteConfirmProps {
  isOpen: boolean;
  title: string;
  description: string;
  onClose: () => void;
  onConfirm: () => void;
}

/**
 * DNS 页删除确认框（服务器 / 规则共用，ADR-0005 P0-4b）。
 *
 * 由编辑 Sheet 的删除入口触发（先收起 Sheet 再弹确认，避免双层遮罩叠放）。
 */
export function DnsDeleteConfirm({ isOpen, title, description, onClose, onConfirm }: DnsDeleteConfirmProps) {
  return (
    <ConfirmDialog opened={isOpen} title={title} danger confirmText="删除" onConfirm={onConfirm} onClose={onClose}>
      <p className="break-words">{description}</p>
    </ConfirmDialog>
  );
}
