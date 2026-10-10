import { create } from "zustand";

/**
 * 轻量通知：client-core 保持 UI 无关，未注册适配器时写入 zustand 队列。
 * 客户端双端统一由 @pp/ui ToastRegion 消费静态队列；桌面最多三条、移动最新一条。
 * 不使用 view-transition，避免 WebKitGTK / WSL 软渲染的历史崩溃。
 * setToastHandler 保留为适配器扩展点，现有客户端不需要注册。
 */

export type ToastKind = "success" | "warning" | "danger";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

/** toast 渲染适配器：壳应用注册后接管全部 toast 输出。 */
export type ToastHandler = (kind: ToastKind, message: string) => void;

/** 同屏最大条数（超出丢弃最旧）。 */
const MAX_TOASTS = 3;
/** 自动消失时长（毫秒）。 */
const TOAST_DURATION_MS = 4000;

let nextId = 0;

/** 已注册的 toast 渲染适配器；`null` 时走 zustand 静态 store 兜底。 */
let toastHandler: ToastHandler | null = null;

/** 注册可选适配器；传 null 恢复静态队列。 */
export function setToastHandler(handler: ToastHandler | null): void {
  toastHandler = handler;
}

interface ToastStore {
  toasts: ToastItem[];
  pushToast: (kind: ToastKind, message: string) => void;
  dismissToast: (id: number) => void;
}

export const useToastStore = create<ToastStore>((set) => ({
  toasts: [],
  pushToast: (kind, message) => {
    const id = ++nextId;
    set((state) => {
      const next = [...state.toasts, { id, kind, message }];
      return { toasts: next.length > MAX_TOASTS ? next.slice(next.length - MAX_TOASTS) : next };
    });
    setTimeout(() => {
      useToastStore.getState().dismissToast(id);
    }, TOAST_DURATION_MS);
  },
  dismissToast: (id) => {
    set((state) => ({ toasts: state.toasts.filter((t) => t.id !== id) }));
  },
}));

/** 发出一条 toast：已注册适配器则委派，否则落入静态 store 兜底。 */
function emitToast(kind: ToastKind, message: string): void {
  if (toastHandler) {
    toastHandler(kind, message);
    return;
  }
  useToastStore.getState().pushToast(kind, message);
}

/** 成功提示。 */
export function toastSuccess(message: string): void {
  emitToast("success", message);
}

/** 警告提示（保存等操作的非阻塞提示）。 */
export function toastWarning(message: string): void {
  emitToast("warning", message);
}

/** 错误提示。 */
export function toastError(message: string): void {
  emitToast("danger", message);
}
