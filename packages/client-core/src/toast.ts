import { create } from "zustand";

/**
 * 轻量 toast 通知（适配器模式，client-core 不依赖任何具体 UI 库）。
 *
 * 渲染策略由壳应用（desktop / mobile）在启动时经 `setToastHandler` 注册：
 * - desktop：默认注册 HeroUI 原生 toast；`PP_TOAST_MODE=static`（兼容 `safe`）
 *   环境变量（经 `toast_mode_override` 命令读取）下不注册，走下方 zustand
 *   静态 store，由 desktop 的 `<Toaster />` 消费渲染。
 * - mobile：注册基于 Konsta UI Toast 的 `<MobileToaster />`。
 *
 * 背景：HeroUI 3.2.2 的 `ToastProvider` 渲染 toast 时调用
 * `document.startViewTransition()`（`@heroui/react` 的 toast-queue 实现），
 * WebKitGTK 2.52.5 在 WSL 软渲染下执行 view-transition 会 SIGSEGV 直接退出整个
 * 进程（dmesg 实证），每次 toast（保存成功/代理启停）应用即崩溃；且移动端
 * 已迁移至 Konsta UI，client-core 不应再传递依赖 `@heroui/react`。
 *
 * 未注册 handler 时的兜底：所有 toast 走 zustand store（右下角堆叠、最多同屏
 * 3 条、4 秒自动消失），壳应用渲染各自的静态 Toaster 消费。
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

/**
 * 注册 toast 渲染适配器（壳应用启动时调用一次）。
 *
 * 注意：注册生效前的 toast 会落入静态 store；若壳应用此时不渲染静态
 * Toaster（如 desktop 的 HeroUI 原生路径），这些早期 toast 不可见——可接受，
 * toast 均由用户交互触发，正常时序晚于启动注册。
 */
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
