import type { SubscriptionView } from "@pp/client-core";
import { ConfirmDialog } from "./ui";

interface SubscriptionDeleteConfirmProps {
  /** 待删除订阅；`null` = 关闭。 */
  sub: SubscriptionView | null;
  /** 该订阅是否为当前生效订阅（用于确认框内追加提示）。 */
  isActive: boolean;
  /** 删除进行中（确认按钮 loading）。 */
  busy: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

/**
 * 删除订阅确认框（ADR-0003 M5.6）。
 *
 * 删除的是当前生效订阅时，在确认文案中追加提示（删除后生效标记由 Rust 侧清除，
 * 页面 toast 追加「请重新选择」引导，对齐任务反馈文案）。
 */
export function SubscriptionDeleteConfirm({ sub, isActive, busy, onClose, onConfirm }: SubscriptionDeleteConfirmProps) {
  return (
    <ConfirmDialog
      opened={sub !== null}
      title="删除订阅"
      danger
      busy={busy}
      confirmText="删除"
      onConfirm={onConfirm}
      onClose={onClose}
    >
      <p className="break-words">确定删除订阅「{sub ? sub.name : ""}」吗？该操作不可撤销。</p>
      {isActive && <p className="mt-1 text-sm text-amber-500">该订阅为当前生效订阅，删除后请重新选择生效订阅。</p>}
    </ConfirmDialog>
  );
}
