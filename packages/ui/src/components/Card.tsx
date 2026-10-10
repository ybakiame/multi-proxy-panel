/**
 * Card（T0 纯样式，无平台分叉）。
 * 复合结构对齐桌面 HeroUI `Card` / 移动适配层同名 API：
 * `Card` + `Card.Header/Title/Description/Content/Footer`。
 */
import type { ComponentProps, ReactNode } from "react";
import { IS_MOBILE } from "../platform.ts";
import { cx } from "../utils.ts";

interface CardSectionProps extends ComponentProps<"div"> {
  className?: string;
  children?: ReactNode;
}

function CardRoot({ className, children, ...rest }: CardSectionProps) {
  // 桌面：HeroUI 表面卡（圆角 xl + surface 阴影）；移动：iOS 分组卡
  // （大圆角、无阴影，Konsta Card 迁移后的等价形态）。
  return (
    <div
      {...rest}
      className={cx(
        "bg-surface text-foreground",
        IS_MOBILE ? "rounded-2xl shadow-surface" : "rounded-xl shadow-surface",
        className,
      )}
    >
      {children}
    </div>
  );
}

function CardHeader({ className, children }: CardSectionProps) {
  return <div className={cx("flex flex-col gap-0.5 px-4 pt-4", className)}>{children}</div>;
}

function CardTitle({ className, children }: CardSectionProps) {
  return <h3 className={cx("text-base font-semibold text-foreground", className)}>{children}</h3>;
}

function CardDescription({ className, children }: CardSectionProps) {
  return <p className={cx("text-sm text-muted", className)}>{children}</p>;
}

function CardContent({ className, children }: CardSectionProps) {
  return <div className={cx("p-4", className)}>{children}</div>;
}

function CardFooter({ className, children }: CardSectionProps) {
  return <div className={cx("px-4 pb-4", className)}>{children}</div>;
}

export const Card = Object.assign(CardRoot, {
  Header: CardHeader,
  Title: CardTitle,
  Description: CardDescription,
  Content: CardContent,
  Footer: CardFooter,
});
