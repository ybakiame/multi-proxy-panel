import { useToastStore, type ToastKind } from "../toast";

/**
 * 轻量 toast 渲染组件。
 *
 * 自实现（替代 HeroUI `ToastProvider`）：纯静态渲染，零动画、零 view-transition、
 * 零 HeroUI 依赖，规避 WebKitGTK 在 WSL 软渲染下执行 `startViewTransition` 的
 * SIGSEGV（见 `../toast.ts`）。样式与深色主题一致（HeroUI 主题 token），固定
 * 右下角纵向堆叠，每条左侧带 4px 色条（success=绿 / warning=黄 / danger=红）与
 * 「×」关闭按钮。
 */
const KIND_BAR: Record<ToastKind, string> = {
  success: "bg-success",
  warning: "bg-warning",
  danger: "bg-danger",
};

export function Toaster() {
  const toasts = useToastStore((s) => s.toasts);
  const dismissToast = useToastStore((s) => s.dismissToast);

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-50 flex w-80 max-w-[calc(100vw-2rem)] flex-col gap-2">
      {toasts.map((toast) => (
        <div
          key={toast.id}
          className="pointer-events-auto flex items-stretch overflow-hidden rounded-lg border border-border/70 bg-surface shadow-lg"
        >
          <span className={`w-1 shrink-0 ${KIND_BAR[toast.kind]}`} aria-hidden="true" />
          <p className="flex-1 px-3 py-2.5 text-sm leading-snug text-foreground">{toast.message}</p>
          <button
            type="button"
            aria-label="关闭"
            className="px-2 text-muted hover:text-foreground"
            onClick={() => dismissToast(toast.id)}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
