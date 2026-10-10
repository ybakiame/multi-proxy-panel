import type { ReactNode } from "react";
import {
  CheckCircleIcon,
  ExclamationTriangleIcon as ExclamationCircleIcon,
  ExclamationTriangleIcon,
  InformationCircleIcon,
} from "../icons";

export type InlineAlertKind = "success" | "warning" | "danger" | "info";

interface InlineAlertProps {
  kind?: InlineAlertKind;
  /** 标题（可选，加粗一行）。 */
  title?: ReactNode;
  children?: ReactNode;
  className?: string;
}

const KIND_STYLES: Record<InlineAlertKind, { wrap: string; icon: typeof InformationCircleIcon }> = {
  success: {
    wrap: "bg-green-500/10 text-green-700 dark:text-green-400",
    icon: CheckCircleIcon,
  },
  warning: {
    wrap: "bg-amber-500/10 text-amber-700 dark:text-amber-400",
    icon: ExclamationTriangleIcon,
  },
  danger: {
    wrap: "bg-red-500/10 text-red-700 dark:text-red-400",
    icon: ExclamationCircleIcon,
  },
  info: {
    wrap: "bg-primary/10 text-primary",
    icon: InformationCircleIcon,
  },
};

/**
 * 内联警示横幅（T0，共享语义令牌）。
 *
 * 用于页面内的错误/警告/提示展示（如加载失败、功能说明），非 toast。
 */
export function InlineAlert({ kind = "info", title, children, className = "" }: InlineAlertProps) {
  const styles = KIND_STYLES[kind];
  const Icon = styles.icon;
  return (
    <div role="alert" className={`flex items-start gap-2 rounded-xl p-3 text-sm ${styles.wrap} ${className}`}>
      <Icon className="mt-0.5 size-5 shrink-0" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        {title ? <div className="font-medium">{title}</div> : null}
        {children ? <div className={title ? "mt-0.5 opacity-90" : ""}>{children}</div> : null}
      </div>
    </div>
  );
}
