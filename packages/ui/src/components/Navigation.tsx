import type { ComponentProps, ReactNode } from "react";
import { cx } from "../utils";

export function Segmented({ strong: _strong, className, ...props }: ComponentProps<"div"> & { strong?: boolean }) {
  return <div role="group" className={cx("flex gap-1 rounded-xl bg-default p-1", className)} {...props} />;
}
export function SegmentedButton({ active, className, ...props }: ComponentProps<"button"> & { active?: boolean }) {
  return (
    <button
      type="button"
      aria-pressed={active}
      className={cx(
        "min-h-11 min-w-0 flex-1 cursor-pointer rounded-lg px-2 text-sm font-medium outline-none transition-colors active:opacity-70 disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-focus",
        active ? "bg-surface text-primary shadow-sm" : "text-muted",
        className,
      )}
      {...props}
    />
  );
}
export function Navbar({ title, left, right }: { title: string; left?: ReactNode; right?: ReactNode }) {
  return (
    <header className="sticky top-0 z-20 border-b border-separator bg-surface/90 pt-[env(safe-area-inset-top)] backdrop-blur-xl">
      <div className="grid min-h-14 grid-cols-[1fr_auto_1fr] items-center gap-2 px-4">
        <div>{left}</div>
        <h1 className="text-center text-lg font-semibold text-foreground">{title}</h1>
        <div className="flex justify-end">{right}</div>
      </div>
    </header>
  );
}
export function Fab({ className, ...props }: ComponentProps<"button">) {
  return (
    <button
      type="button"
      className={cx(
        "inline-flex size-14 items-center justify-center rounded-full bg-primary text-primary-foreground shadow-lg outline-none active:scale-95 disabled:opacity-50 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus",
        className,
      )}
      {...props}
    />
  );
}
