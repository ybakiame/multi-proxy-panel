import { Toast } from "konsta/react";
import { useToastStore, type ToastKind } from "@pp/client-core";

/** 各 toast 种类的配色（覆盖 Konsta Toast 默认玻璃底色）。 */
const KIND_CLASSES: Record<ToastKind, string> = {
  success: "bg-green-600 text-white",
  warning: "bg-amber-500 text-black",
  danger: "bg-red-600 text-white",
};

/**
 * 移动端 toast 出口（Konsta UI Toast）。
 *
 * client-core 的 toast 适配器未注册 handler 时默认落入 zustand 静态 store
 * （见 `@pp/client-core` toast.ts），本组件消费该 store：Konsta Toast 单条形态，
 * 同屏只展示最新一条（store 内最多 3 条，前条 4s 自动消失后顺延下一条），
 * 点击立即关闭。挂在 App 根部常驻。
 */
export function MobileToaster() {
  const toasts = useToastStore((s) => s.toasts);
  const dismissToast = useToastStore((s) => s.dismissToast);
  const current = toasts.length > 0 ? toasts[toasts.length - 1] : null;

  return (
    <Toast
      position="center"
      opened={current !== null}
      onClick={() => current && dismissToast(current.id)}
      className={current ? KIND_CLASSES[current.kind] : undefined}
    >
      {current?.message}
    </Toast>
  );
}
