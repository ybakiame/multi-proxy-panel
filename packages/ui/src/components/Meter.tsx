import { Meter as BaseMeter } from "@base-ui/react/meter";
import { createContext, useContext, type ReactNode } from "react";
import { cx } from "../utils";

const Context = createContext<{ label?: string; color: string }>({ color: "bg-accent" });
const COLORS = { accent: "bg-accent", success: "bg-success", warning: "bg-warning", danger: "bg-danger" };
function Root({
  value,
  valueLabel,
  color = "accent",
  size: _size,
  className,
  children,
  "aria-label": label,
}: {
  value: number;
  valueLabel?: string;
  color?: keyof typeof COLORS;
  size?: "sm" | "md";
  className?: string;
  children?: ReactNode;
  "aria-label"?: string;
}) {
  return (
    <Context.Provider value={{ label: valueLabel, color: COLORS[color] }}>
      <BaseMeter.Root
        value={value}
        aria-label={label}
        aria-valuetext={valueLabel}
        className={cx("flex flex-col gap-1", className)}
      >
        {children}
      </BaseMeter.Root>
    </Context.Provider>
  );
}
function Output() {
  const { label } = useContext(Context);
  return <BaseMeter.Value className="text-xs text-muted">{label ? () => label : undefined}</BaseMeter.Value>;
}
function Track({ children }: { children?: ReactNode }) {
  return <BaseMeter.Track className="h-1.5 overflow-hidden rounded-full bg-default">{children}</BaseMeter.Track>;
}
function Fill() {
  const { color } = useContext(Context);
  return <BaseMeter.Indicator className={cx("h-full rounded-full", color)} />;
}
export const Meter = Object.assign(Root, { Output, Track, Fill });
