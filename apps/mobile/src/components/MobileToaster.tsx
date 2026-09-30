import { Toast } from "konsta/react";
import { useToastStore, type ToastKind } from "@pp/client-core";

/** 各 toast 种类的配色（经 colors prop 覆盖 Glass 内层背景；important 防止被
 *  Konsta 默认玻璃色类层叠——twMerge 不识别 ios-light-glass 色标，两者会并存）。 */
const KIND_COLORS: Record<ToastKind, { bgIos: string; bgMaterial: string; textIos: string; textMaterial: string }> = {
  success: { bgIos: "bg-green-600!", bgMaterial: "bg-green-600!", textIos: "text-white!", textMaterial: "text-white!" },
  warning: { bgIos: "bg-amber-500!", bgMaterial: "bg-amber-500!", textIos: "text-black!", textMaterial: "text-black!" },
  danger: { bgIos: "bg-red-600!", bgMaterial: "bg-red-600!", textIos: "text-white!", textMaterial: "text-white!" },
};

/**
 * 移动端 toast 出口（Konsta UI Toast）。
 *
 * client-core 的 toast 适配器未注册 handler 时默认落入 zustand 静态 store
 * （见 `@pp/client-core` toast.ts），本组件消费该 store：Konsta Toast 单条形态，
 * 同屏只展示最新一条（store 内最多 3 条，前条 4s 自动消失后顺延下一条），
 * 点击立即关闭。挂在 App 根部常驻。
 * 说明：背景色必须走 `colors` prop（作用于 Glass 内层）；挂在根 className 会在
 * 全宽容器上铺出一整条色块（根为 flex 全宽，内层 Glass 才是可见气泡）。
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
      colors={current ? KIND_COLORS[current.kind] : undefined}
    >
      {current?.message}
    </Toast>
  );
}
