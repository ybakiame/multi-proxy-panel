/**
 * Spinner（T0 纯样式）：SVG 旋转加载指示。
 * `color`: `accent` = 主题主色（默认）；`current` / `white` = 跟随/固定文字色
 * （实心按钮内嵌 spinner 用 `white`）。
 */
import { cx } from "../utils.ts";

interface SpinnerProps {
  size?: "sm" | "md" | "lg";
  color?: "accent" | "current" | "white";
  className?: string;
  "aria-hidden"?: boolean | "true" | "false";
}

const SIZES = { sm: "size-4", md: "size-6", lg: "size-8" } as const;
const COLORS = { accent: "text-accent", current: "text-current", white: "text-white" } as const;

export function Spinner({ size = "md", color = "accent", className, ...rest }: SpinnerProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      role="status"
      className={cx("animate-spin", SIZES[size], COLORS[color], className)}
      {...rest}
    >
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-90" fill="currentColor" d="M4 12a8 8 0 0 1 8-8V0C5.373 0 0 5.373 0 12h4z" />
    </svg>
  );
}
