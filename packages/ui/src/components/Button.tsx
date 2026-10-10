/**
 * Button（T1，Base UI Button 封装）。
 * props 沿用 HeroUI/移动适配层风格：variant(primary/secondary/tertiary/danger/ghost)、
 * size(sm/md/lg)、isPending、isDisabled、isIconOnly、onPress、fullWidth。
 * 平台皮肤：移动端更大触控高度 + active 按压反馈（波纹按 ADR-0013 后置）。
 */
import { Button as BaseButton } from "@base-ui/react/button";
import type { ComponentProps, ReactNode } from "react";
import { IS_MOBILE } from "../platform.ts";
import { cx } from "../utils.ts";
import { Spinner } from "./Spinner.tsx";

export type ButtonVariant = "primary" | "secondary" | "tertiary" | "danger" | "ghost";
export type ButtonSize = "sm" | "md" | "lg";

/** 实心填充（primary/danger）时内嵌 spinner 用白色，其余跟随主题主色。 */
const FILLED_VARIANTS: ReadonlySet<ButtonVariant> = new Set(["primary", "danger"]);

const VARIANT_CLASSES: Record<ButtonVariant, string> = {
  primary: IS_MOBILE
    ? "bg-primary text-primary-foreground active:bg-primary/90"
    : "bg-accent text-accent-foreground hover:bg-accent-hover",
  secondary: IS_MOBILE
    ? "bg-default text-default-foreground active:bg-default-hover"
    : "bg-default text-default-foreground hover:bg-default-hover",
  tertiary: "bg-default-soft text-default-soft-foreground hover:bg-default-soft-hover",
  danger: IS_MOBILE
    ? "bg-danger text-danger-foreground active:bg-danger-hover"
    : "bg-danger text-danger-foreground hover:bg-danger-hover",
  ghost: "bg-transparent text-foreground hover:bg-default-soft active:bg-default-soft-hover",
};

const SIZE_CLASSES: Record<ButtonSize, string> = IS_MOBILE
  ? { sm: "h-9 rounded-lg px-3 text-sm", md: "h-11 rounded-lg px-4 text-base", lg: "h-13 rounded-xl px-5 text-base" }
  : { sm: "h-8 rounded-lg px-3 text-sm", md: "h-10 rounded-lg px-4 text-sm", lg: "h-12 rounded-xl px-5 text-base" };

interface ButtonProps extends Omit<ComponentProps<typeof BaseButton>, "className"> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  /** 禁用（等价 HeroUI `isDisabled`）。 */
  isDisabled?: boolean;
  /** 进行中：按钮内嵌 spinner 并禁用（等价 HeroUI `isPending`）。 */
  isPending?: boolean;
  /** 仅图标按钮（正方形、去水平内边距）。 */
  isIconOnly?: boolean;
  /** 占满容器宽度。 */
  fullWidth?: boolean;
  /** 点击回调（等价 HeroUI `onPress`，内部转 onClick）。 */
  onPress?: () => void;
  className?: string;
  children?: ReactNode;
}

export function Button({
  variant = "primary",
  size = "md",
  isDisabled = false,
  isPending = false,
  isIconOnly = false,
  fullWidth = false,
  onPress,
  className,
  children,
  ...rest
}: ButtonProps) {
  return (
    <BaseButton
      disabled={isDisabled || isPending || rest.disabled}
      onClick={onPress}
      className={cx(
        "inline-flex cursor-pointer items-center justify-center gap-2 font-medium transition-colors",
        "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus",
        "disabled:cursor-not-allowed disabled:opacity-50",
        IS_MOBILE && "active:scale-[0.98]",
        VARIANT_CLASSES[variant],
        SIZE_CLASSES[size],
        isIconOnly && "aspect-square !px-0",
        fullWidth && "w-full",
        className,
      )}
      {...rest}
    >
      {isPending ? (
        <>
          <Spinner size="sm" color={FILLED_VARIANTS.has(variant) ? "white" : "accent"} aria-hidden />
          {isIconOnly ? null : children}
        </>
      ) : (
        children
      )}
    </BaseButton>
  );
}
