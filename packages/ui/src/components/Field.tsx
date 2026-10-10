/**
 * Field（T1，Base UI Field 封装）：表单域容器复合结构。
 * `Field.Root/Label/Description/Error`；控件直接放 `Input`（Base UI 自动
 * 完成 aria-describedby / invalid 接线，无需显式 Control）。
 */
import { Field as BaseField } from "@base-ui/react/field";
import type { ComponentProps, ReactNode } from "react";
import { cx } from "../utils.ts";

interface FieldSectionProps {
  className?: string;
  children?: ReactNode;
}

function FieldRoot({ className, children }: FieldSectionProps) {
  return <BaseField.Root className={cx("flex flex-col gap-1.5", className)}>{children}</BaseField.Root>;
}

function FieldLabel({ className, children }: FieldSectionProps) {
  return <BaseField.Label className={cx("text-sm font-medium text-foreground", className)}>{children}</BaseField.Label>;
}

function FieldDescription({ className, children }: FieldSectionProps) {
  return <BaseField.Description className={cx("text-xs text-muted", className)}>{children}</BaseField.Description>;
}

interface FieldErrorProps extends Omit<ComponentProps<typeof BaseField.Error>, "className"> {
  className?: string;
}

function FieldError({ className, ...rest }: FieldErrorProps) {
  return <BaseField.Error className={cx("text-xs text-danger", className)} {...rest} />;
}

export const Field = Object.assign(FieldRoot, {
  Label: FieldLabel,
  Description: FieldDescription,
  Error: FieldError,
});
