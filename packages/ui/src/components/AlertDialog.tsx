/**
 * AlertDialog（T2 Overlay 族）——确认/警示对话框（Base UI AlertDialog），
 * 双端同为居中模态（移动 Konsta Dialog 也是居中形态，无平台分叉）。
 * 嵌套在 Modal 内时 Base UI 自动识别嵌套（子级不重复渲染 backdrop）。
 *
 * 复合 API 对齐 HeroUI v3 桌面用法（Backdrop/Container/Dialog/Header/Icon/Heading/Body/
 * Footer/CloseTrigger），样式规格镜像 @heroui/styles alert-dialog.css。
 * CloseTrigger 等价 HeroUI 的 `<Button slot="close">`（渲染为 tertiary Button 并自动关闭）。
 *
 * 另导出便捷封装 ConfirmDialog（对齐移动 ui.tsx 旧 API：opened/title/confirmText/
 * cancelText/danger/busy/onConfirm/onClose），双端通用。
 */
import { AlertDialog as BaseAlertDialog } from "@base-ui/react/alert-dialog";
import type { ReactNode } from "react";
import { CheckCircleIcon, ExclamationTriangleIcon, InformationCircleIcon } from "../icons.tsx";
import { cx } from "../utils.ts";
import { Button } from "./Button.tsx";

export interface AlertDialogBackdropProps {
  isOpen: boolean;
  onOpenChange?: (open: boolean) => void;
  children?: ReactNode;
}

interface SectionProps {
  className?: string;
  children?: ReactNode;
}

export type AlertDialogStatus = "default" | "accent" | "success" | "warning" | "danger";
export type AlertDialogSize = "xs" | "sm" | "md" | "lg";

const SIZE_MAXW: Record<AlertDialogSize, string> = {
  xs: "max-w-xs",
  sm: "max-w-sm",
  md: "max-w-md",
  lg: "max-w-lg",
};

const ICON_COLORS: Record<AlertDialogStatus, string> = {
  default: "bg-default text-foreground",
  accent: "bg-accent-soft text-accent-soft-foreground",
  success: "bg-success-soft text-success-soft-foreground",
  warning: "bg-warning-soft text-warning-soft-foreground",
  danger: "bg-danger-soft text-danger-soft-foreground",
};

function statusIcon(status: AlertDialogStatus) {
  if (status === "warning" || status === "danger") return <ExclamationTriangleIcon className="size-5" />;
  if (status === "success") return <CheckCircleIcon className="size-5" />;
  return <InformationCircleIcon className="size-5" />;
}

function Backdrop({ isOpen, onOpenChange, children }: AlertDialogBackdropProps) {
  return (
    <BaseAlertDialog.Root open={isOpen} onOpenChange={(open) => onOpenChange?.(open)}>
      <BaseAlertDialog.Portal>
        <BaseAlertDialog.Backdrop
          className={cx(
            "fixed inset-0 z-50 bg-backdrop transition-opacity duration-150",
            "data-[starting-style]:opacity-0 data-[ending-style]:opacity-0",
          )}
        />
        <BaseAlertDialog.Popup
          className={cx(
            "pointer-events-none fixed inset-0 z-50 flex items-center justify-center p-4 sm:p-10",
            "transition-[opacity,scale] duration-200",
            "data-[starting-style]:scale-95 data-[starting-style]:opacity-0",
            "data-[ending-style]:scale-95 data-[ending-style]:opacity-0",
          )}
        >
          {children}
        </BaseAlertDialog.Popup>
      </BaseAlertDialog.Portal>
    </BaseAlertDialog.Root>
  );
}

function Container({ size = "sm", className, children }: SectionProps & { size?: AlertDialogSize }) {
  return <div className={cx("w-full", SIZE_MAXW[size], className)}>{children}</div>;
}

function Dialog({ className, children }: SectionProps) {
  return (
    <div
      className={cx(
        "pointer-events-auto relative flex max-h-full w-full flex-col overflow-clip rounded-3xl bg-overlay p-6 shadow-overlay outline-none",
        className,
      )}
    >
      {children}
    </div>
  );
}

/** 顶部区：icon + 标题纵向堆叠（HeroUI alert-dialog__header 为 flex-col gap-3）。 */
function Header({ className, children }: SectionProps) {
  return <div className={cx("flex flex-col gap-3", className)}>{children}</div>;
}

function Icon({ status = "default", className }: { status?: AlertDialogStatus; className?: string }) {
  return (
    <div
      className={cx(
        "flex size-10 shrink-0 items-center justify-center rounded-3xl select-none",
        ICON_COLORS[status],
        className,
      )}
    >
      {statusIcon(status)}
    </div>
  );
}

function Heading({ className, children }: SectionProps) {
  return (
    <BaseAlertDialog.Title className={cx("text-base font-medium text-foreground", className)}>
      {children}
    </BaseAlertDialog.Title>
  );
}

function Body({ className, children }: SectionProps) {
  return (
    <div
      className={cx(
        "mt-3 min-h-0 flex-1 overflow-y-auto overscroll-contain text-sm leading-[1.43] text-muted",
        className,
      )}
    >
      {children}
    </div>
  );
}

function Footer({ className, children }: SectionProps) {
  return <div className={cx("mt-4 flex flex-row items-center justify-end gap-2", className)}>{children}</div>;
}

/** 等价 HeroUI `<Button slot="close">`：tertiary 样式的关闭按钮。 */
function CloseTrigger({ children }: { children?: ReactNode }) {
  return <BaseAlertDialog.Close render={<Button variant="tertiary" />}>{children}</BaseAlertDialog.Close>;
}

export const AlertDialog = {
  Backdrop,
  Container,
  Dialog,
  Header,
  Icon,
  Heading,
  Body,
  Footer,
  CloseTrigger,
};

/* -------------------------------------------------------------------------- */
/* ConfirmDialog：双端通用确认弹窗（对齐移动 ui.tsx 旧 API）                      */
/* -------------------------------------------------------------------------- */

export interface ConfirmDialogProps {
  opened: boolean;
  title: ReactNode;
  confirmText?: string;
  cancelText?: string;
  /** 确认按钮用 danger 配色。 */
  danger?: boolean;
  /** 进行中：确认按钮转菊花，且禁止关闭（对齐 Konsta 行为）。 */
  busy?: boolean;
  onConfirm: () => void;
  onClose: () => void;
  children?: ReactNode;
}

export function ConfirmDialog({
  opened,
  title,
  confirmText = "确认",
  cancelText = "取消",
  danger = false,
  busy = false,
  onConfirm,
  onClose,
  children,
}: ConfirmDialogProps) {
  return (
    <Backdrop isOpen={opened} onOpenChange={(open) => !open && !busy && onClose()}>
      <Container size="sm">
        <Dialog>
          <Header>
            <Heading>{title}</Heading>
          </Header>
          {children != null && <Body>{children}</Body>}
          <Footer>
            <Button variant="tertiary" isDisabled={busy} onPress={onClose}>
              {cancelText}
            </Button>
            <Button variant={danger ? "danger" : "primary"} isPending={busy} onPress={onConfirm}>
              {confirmText}
            </Button>
          </Footer>
        </Dialog>
      </Container>
    </Backdrop>
  );
}
