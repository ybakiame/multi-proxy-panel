import type { ReactNode } from "react";
import { IS_MOBILE } from "../platform";
import { cx } from "../utils";

export interface ToastItem {
  id: number;
  kind: "success" | "warning" | "danger";
  message: ReactNode;
}
const COLORS = { success: "bg-success", warning: "bg-warning", danger: "bg-danger" };
export function ToastRegion({ toasts, onDismiss }: { toasts: ToastItem[]; onDismiss: (id: number) => void }) {
  return (
    <div
      role="region"
      aria-label="通知"
      className={cx(
        "pointer-events-none fixed z-[70] flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2",
        IS_MOBILE ? "bottom-[calc(7rem+env(safe-area-inset-bottom))] left-1/2 -translate-x-1/2" : "right-4 bottom-4",
      )}
    >
      {toasts.map((toast) => (
        <div
          key={toast.id}
          role="status"
          aria-live="polite"
          aria-atomic="true"
          className="pointer-events-auto flex items-stretch overflow-hidden rounded-xl border border-border bg-surface text-foreground shadow-overlay"
        >
          <span aria-hidden className={cx("w-1 shrink-0", COLORS[toast.kind])} />
          <p className="min-w-0 flex-1 break-words px-3 py-3 text-sm">{toast.message}</p>
          <button
            type="button"
            aria-label="关闭通知"
            onClick={() => onDismiss(toast.id)}
            className="min-w-11 text-muted outline-none focus-visible:outline-2 focus-visible:outline-focus"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
