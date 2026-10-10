/**
 * Input（T1，Base UI Input 封装）：文本输入。
 * 置于 `Field.Root` 内时自动获得 aria 接线（Base UI Field 集成）。
 * `inputClassName` / `textareaClassName` 供仍需裸 input/textarea 的场景
 * （沿用移动适配层既有模式）。
 */
import { Input as BaseInput } from "@base-ui/react/input";
import type { ComponentProps } from "react";
import { IS_MOBILE } from "../platform.ts";
import { cx } from "../utils.ts";

const INPUT_BASE = IS_MOBILE
  ? "h-12 w-full rounded-lg border border-border bg-field-background px-3 text-base text-field-foreground shadow-field outline-none transition-colors placeholder:text-field-placeholder hover:bg-field-hover focus:border-field-border-focus disabled:cursor-not-allowed disabled:opacity-50"
  : "h-10 w-full rounded-field border border-field-border bg-field-background px-3 text-sm text-field-foreground shadow-field outline-none transition-colors placeholder:text-field-placeholder hover:bg-field-hover focus:border-field-border-focus disabled:cursor-not-allowed disabled:opacity-50";

/** 裸 input 统一样式（与 `Input` 组件同源）。 */
export const inputClassName = INPUT_BASE;

/** 裸 textarea 变体：去固定行高、加纵向内边距。 */
export const textareaClassName = cx(INPUT_BASE, "h-auto min-h-24 py-2");

interface InputProps extends Omit<ComponentProps<typeof BaseInput>, "className"> {
  className?: string;
}

export function Input({ className, ...rest }: InputProps) {
  return <BaseInput className={cx(INPUT_BASE, className)} {...rest} />;
}
