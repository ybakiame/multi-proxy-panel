import { toast } from "@heroui/react";

/**
 * 全局 toast 通知便捷封装。
 *
 * 底层使用 HeroUI 官方 `toast` 单例（`@heroui/react` 3.2.2），由应用根组件
 * 挂载的 `<ToastProvider />` 消费（Provider 未传 queue 时默认读取该全局队列）。
 * 默认右下角堆叠展示、最多同屏 3 条、4 秒自动消失。
 */

/** 成功提示。 */
export function toastSuccess(message: string): void {
  toast.success(message);
}

/** 警告提示（保存等操作的非阻塞提示）。 */
export function toastWarning(message: string): void {
  toast.warning(message);
}

/** 错误提示。 */
export function toastError(message: string): void {
  toast.danger(message);
}
