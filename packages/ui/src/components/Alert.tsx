/**
 * Alert（T0 复合）：内联提示条，对齐桌面 HeroUI `Alert` 复合用法
 * （`status` + `Alert.Indicator/Content/Title/Description`，桌面现存 38 处）。
 * Indicator 图标随 status 自动选择（default/accent→信息，success→对勾，
 * warning/danger→三角警示），也可经 children 自定义。
 */
import { createContext, useContext, type ReactNode } from "react";
import { cx } from "../utils.ts";

export type AlertStatus = "default" | "accent" | "success" | "warning" | "danger";

const AlertStatusContext = createContext<AlertStatus>("default");

const ROOT_COLORS: Record<AlertStatus, string> = {
  default: "bg-default-soft text-default-soft-foreground",
  accent: "bg-accent-soft text-accent-soft-foreground",
  success: "bg-success-soft text-success-soft-foreground",
  warning: "bg-warning-soft text-warning-soft-foreground",
  danger: "bg-danger-soft text-danger-soft-foreground",
};

const INDICATOR_COLORS: Record<AlertStatus, string> = {
  default: "text-foreground",
  accent: "text-accent",
  success: "text-success",
  warning: "text-warning",
  danger: "text-danger",
};

/* Heroicons 24/outline 路径（information-circle / check-circle / exclamation-triangle）。 */
const ICON_PATHS: Record<"info" | "check" | "warning", string> = {
  info: "M11.25 11.25l.041-.02a.75.75 0 0 1 1.063.852l-.708 2.836a.75.75 0 0 0 1.063.853l.041-.021M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Zm-9-3.75h.008v.008H12V8.25Z",
  check: "M9 12.75 11.25 15 15 9.75M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z",
  warning:
    "M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z",
};

function iconKind(status: AlertStatus): keyof typeof ICON_PATHS {
  if (status === "success") return "check";
  if (status === "warning" || status === "danger") return "warning";
  return "info";
}

interface AlertSectionProps {
  className?: string;
  children?: ReactNode;
}

interface AlertRootProps extends AlertSectionProps {
  status?: AlertStatus;
}

function AlertRoot({ status = "default", className, children }: AlertRootProps) {
  return (
    <AlertStatusContext.Provider value={status}>
      <div role="alert" className={cx("flex gap-3 rounded-xl p-4", ROOT_COLORS[status], className)}>
        {children}
      </div>
    </AlertStatusContext.Provider>
  );
}

function AlertIndicator({ className, children }: AlertSectionProps) {
  const status = useContext(AlertStatusContext);
  return (
    <div className={cx("shrink-0", INDICATOR_COLORS[status], className)}>
      {children ?? (
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" className="size-5">
          <path strokeLinecap="round" strokeLinejoin="round" d={ICON_PATHS[iconKind(status)]} />
        </svg>
      )}
    </div>
  );
}

function AlertContent({ className, children }: AlertSectionProps) {
  return <div className={cx("flex min-w-0 flex-col gap-1", className)}>{children}</div>;
}

function AlertTitle({ className, children }: AlertSectionProps) {
  return <p className={cx("text-sm font-medium", className)}>{children}</p>;
}

function AlertDescription({ className, children }: AlertSectionProps) {
  return <p className={cx("text-sm opacity-90", className)}>{children}</p>;
}

export const Alert = Object.assign(AlertRoot, {
  Indicator: AlertIndicator,
  Content: AlertContent,
  Title: AlertTitle,
  Description: AlertDescription,
});
