import { Tabs as BaseTabs } from "@base-ui/react/tabs";
import type { ComponentProps, ReactNode } from "react";
import { cx } from "../utils";

function Root(props: ComponentProps<typeof BaseTabs.Root>) {
  return <BaseTabs.Root {...props} />;
}
function ListContainer({ children }: { children?: ReactNode }) {
  return <div className="overflow-x-auto">{children}</div>;
}
function List({
  className,
  ...props
}: Omit<ComponentProps<typeof BaseTabs.List>, "className"> & { className?: string }) {
  return <BaseTabs.List className={cx("inline-flex gap-1 rounded-xl bg-default p-1", className)} {...props} />;
}
function Tab({
  id,
  className,
  ...props
}: Omit<ComponentProps<typeof BaseTabs.Tab>, "className" | "value"> & { id: string; className?: string }) {
  return (
    <BaseTabs.Tab
      value={id}
      className={cx(
        "relative min-h-9 cursor-pointer rounded-lg px-4 text-sm text-muted outline-none data-active:bg-surface data-active:text-foreground data-active:shadow-sm focus-visible:outline-2 focus-visible:outline-focus",
        className,
      )}
      {...props}
    />
  );
}
function Panel({
  id,
  className,
  ...props
}: Omit<ComponentProps<typeof BaseTabs.Panel>, "className" | "value"> & { id: string; className?: string }) {
  return <BaseTabs.Panel value={id} className={className} {...props} />;
}
// 选中态由 Tab 的 data-active 绘制，保持存量复合调用点兼容。
function Indicator() {
  return null;
}
export const Tabs = Object.assign(Root, { ListContainer, List, Tab, Panel, Indicator });
