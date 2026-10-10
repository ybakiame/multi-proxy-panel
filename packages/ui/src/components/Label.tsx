/**
 * Label（T0 纯样式）：表单标签，对齐 HeroUI `Label` 的裸用法（桌面现存 68 处）。
 * 与 `Field.Root` 内联用时建议改用 `Field.Label`（自动 aria 接线）。
 */
import type { ComponentProps } from "react";
import { cx } from "../utils.ts";

export function Label({ className, ...rest }: ComponentProps<"label">) {
  return <label className={cx("text-sm font-medium text-foreground", className)} {...rest} />;
}
