import { Avatar as BaseAvatar } from "@base-ui/react/avatar";
import type { ComponentProps, ReactNode } from "react";
import { cx } from "../utils";

function Root({
  size = "md",
  className,
  ...props
}: Omit<ComponentProps<typeof BaseAvatar.Root>, "className"> & { size?: "sm" | "md"; className?: string }) {
  return (
    <BaseAvatar.Root
      className={cx(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full",
        size === "sm" ? "size-8" : "size-10",
        className,
      )}
      {...props}
    />
  );
}
function Image(props: ComponentProps<typeof BaseAvatar.Image>) {
  return <BaseAvatar.Image className="size-full object-cover" {...props} />;
}
function Fallback({ children, color: _color }: { children?: ReactNode; color?: "accent" }) {
  return (
    <BaseAvatar.Fallback className="flex size-full items-center justify-center bg-accent-soft text-xs text-accent">
      {children}
    </BaseAvatar.Fallback>
  );
}
export const Avatar = Object.assign(Root, { Image, Fallback });
