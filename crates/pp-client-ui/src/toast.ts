import { toast as heroToast } from "@heroui/react";
import { create } from "zustand";

/**
 * 轻量 toast 通知（自实现兜底，规避 HeroUI toast 在 WSL 软渲染 WebKitGTK 下的崩溃）。
 *
 * 背景：HeroUI 3.2.2 的 `ToastProvider` 渲染 toast 时调用
 * `document.startViewTransition()`（`@heroui/react` 的 toast-queue 实现），
 * WebKitGTK 2.52.5 在 WSL 软渲染下执行 view-transition 会 SIGSEGV 直接退出整个
 * 进程（dmesg 实证），每次 toast（保存成功/代理启停）应用即崩溃。
 *
 * 方案：双实现委托。桌面端挂载时经 `gpu_acceleration` 命令探测渲染能力——
 * 有 GPU 加速（`setToastMode(true)`）时用 HeroUI 原生 toast（带动画）；
 * 无 GPU / 命令返回前（`heroMode` 默认 `false`，保守安全）走自实现静态 toast：
 * 仅更新 zustand store，由 `<Toaster />` 消费渲染，零动画、零 HeroUI 依赖。
 * 行为对齐：右下角堆叠、最多同屏 3 条、4 秒自动消失。
 */

export type ToastKind = "success" | "warning" | "danger";

export interface ToastItem {
  id: number;
  kind: ToastKind;
  message: string;
}

/** 同屏最大条数（超出丢弃最旧）。 */
const MAX_TOASTS = 3;
/** 自动消失时长（毫秒）。 */
const TOAST_DURATION_MS = 4000;

let nextId = 0;

/** 是否使用 HeroUI 原生 toast（有 GPU 加速时为 `true`）；默认 `false` 走自实现。 */
let heroMode = false;

/** 设置 toast 渲染模式：`hero=true` 用 HeroUI 原生 toast，`false` 走 zustand store。 */
export function setToastMode(hero: boolean): void {
  heroMode = hero;
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

/** 成功提示。 */
export function toastSuccess(message: string): void {
  if (heroMode) {
    heroToast.success(message);
    return;
  }
  useToastStore.getState().pushToast("success", message);
}

/** 警告提示（保存等操作的非阻塞提示）。 */
export function toastWarning(message: string): void {
  if (heroMode) {
    heroToast.warning(message);
    return;
  }
  useToastStore.getState().pushToast("warning", message);
}

/** 错误提示。 */
export function toastError(message: string): void {
  if (heroMode) {
    heroToast.danger(message);
    return;
  }
  useToastStore.getState().pushToast("danger", message);
}
