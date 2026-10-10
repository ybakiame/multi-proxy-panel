/**
 * Separator（T0）：分隔线，Base UI Separator + 语义令牌着色。
 */
import { Separator as BaseSeparator } from "@base-ui/react/separator";
import type { ComponentProps } from "react";
import { cx } from "../utils.ts";

interface SeparatorProps extends Omit<ComponentProps<typeof BaseSeparator>, "className"> {
  className?: string;
}

export function Separator({ className, orientation = "horizontal", ...rest }: SeparatorProps) {
  return (
    <BaseSeparator
      orientation={orientation}
      className={cx("shrink-0 bg-separator", orientation === "horizontal" ? "h-px w-full" : "h-full w-px", className)}
      {...rest}
    />
  );
}
