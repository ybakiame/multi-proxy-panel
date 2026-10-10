/**
 * Chip（T0 纯样式）：软底色徽标，对齐 HeroUI `variant="soft" size="sm"` 用法
 * （桌面现存 45 处全部为该形态）。
 */
import type { ComponentProps, ReactNode } from "react";
import { cx } from "../utils.ts";

export type ChipColor = "default" | "accent" | "danger" | "warning" | "success";

const CHIP_COLORS: Record<ChipColor, string> = {
  default: "bg-default-soft text-default-soft-foreground",
  accent: "bg-accent-soft text-accent-soft-foreground",
  danger: "bg-danger-soft text-danger-soft-foreground",
  warning: "bg-warning-soft text-warning-soft-foreground",
  success: "bg-success-soft text-success-soft-foreground",
};

interface ChipProps extends ComponentProps<"span"> {
  size?: "sm" | "md" | "lg";
  variant?: "soft" | "primary" | "secondary";
  color?: ChipColor;
  className?: string;
  children?: ReactNode;
}

export function Chip({ color = "default", size: _size, variant: _variant, className, children, ...rest }: ChipProps) {
  return (
    <span
      {...rest}
      className={cx(
        "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium",
        CHIP_COLORS[color],
        className,
      )}
    >
      {children}
    </span>
  );
}
