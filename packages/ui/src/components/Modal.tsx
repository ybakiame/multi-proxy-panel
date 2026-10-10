/**
 * Modal（T2 Overlay 族）——桌面居中对话框（Base UI Dialog），移动端底部抽屉
 * （Base UI Drawer，原生滑动关闭手势 + 安全区适配）。
 *
 * 复合 API 对齐 HeroUI v3 桌面用法（Backdrop/Container/Dialog/Header/Heading/Body/
 * Footer/CloseTrigger），迁移期调用点只需改 import；同一 JSX 在移动端渲染为底部抽屉。
 * 样式规格镜像 @heroui/styles modal.css（bg-overlay/shadow-overlay/p-6/rounded-3xl）。
 *
 * 另导出便捷封装 BottomSheet（opened/onClose/title/footer/children，对齐移动 ui.tsx 旧 API），
 * 移动端存量 sheet 迁移时用它可以零 JSX 改动。
 */
import { Dialog as BaseDialog } from "@base-ui/react/dialog";
import { Drawer as BaseDrawer } from "@base-ui/react/drawer";
import type { ComponentType, ReactNode } from "react";
import { XMarkIcon } from "../icons.tsx";
import { IS_MOBILE } from "../platform.ts";
import { cx } from "../utils.ts";

export interface ModalBackdropProps {
  isOpen: boolean;
  onOpenChange?: (open: boolean) => void;
  children?: ReactNode;
}

interface ModalSectionProps {
  className?: string;
  children?: ReactNode;
}

const BACKDROP_CLASSES = cx(
  "fixed inset-0 z-50 bg-backdrop transition-opacity duration-200",
  "data-[starting-style]:opacity-0 data-[ending-style]:opacity-0",
);

/** 桌面：居中对话框（视口滚动容器 + 指针事件只落在面板上）。 */
function DesktopModal({ isOpen, onOpenChange, children }: ModalBackdropProps) {
  return (
    <BaseDialog.Root open={isOpen} onOpenChange={(open) => onOpenChange?.(open)}>
      <BaseDialog.Portal>
        <BaseDialog.Backdrop className={BACKDROP_CLASSES} />
        <BaseDialog.Viewport className="fixed inset-0 z-50 flex justify-center overflow-y-auto p-4 sm:p-10">
          <BaseDialog.Popup
            className={cx(
              "pointer-events-none mt-auto flex w-full justify-center sm:my-auto",
              "transition-[opacity,scale] duration-200",
              "data-[starting-style]:scale-[0.97] data-[starting-style]:opacity-0",
              "data-[ending-style]:scale-[0.97] data-[ending-style]:opacity-0",
            )}
          >
            {children}
          </BaseDialog.Popup>
        </BaseDialog.Viewport>
      </BaseDialog.Portal>
    </BaseDialog.Root>
  );
}

/** 移动：底部抽屉（Drawer 提供下滑关闭；translate 入场 + transform 手势偏移两条轴互不干扰）。 */
function MobileSheet({ isOpen, onOpenChange, children }: ModalBackdropProps) {
  return (
    <BaseDrawer.Root open={isOpen} onOpenChange={(open) => onOpenChange?.(open)}>
      <BaseDrawer.Portal>
        <BaseDrawer.Backdrop className={BACKDROP_CLASSES} />
        <BaseDrawer.Viewport className="fixed inset-0 z-50 flex items-end justify-center">
          <BaseDrawer.Popup
            className={cx(
              "pointer-events-none flex w-full justify-center",
              "transition-transform duration-300 data-[swiping]:transition-none",
              "data-[starting-style]:translate-y-full data-[ending-style]:translate-y-full",
              "[transform:translateY(var(--drawer-swipe-movement-y,0px))]",
            )}
          >
            <BaseDrawer.Content className="flex w-full justify-center">{children}</BaseDrawer.Content>
          </BaseDrawer.Popup>
        </BaseDrawer.Viewport>
      </BaseDrawer.Portal>
    </BaseDrawer.Root>
  );
}

function Backdrop(props: ModalBackdropProps) {
  return IS_MOBILE ? <MobileSheet {...props} /> : <DesktopModal {...props} />;
}

function Container({ className, children }: ModalSectionProps) {
  return <div className={cx("flex w-full justify-center", className)}>{children}</div>;
}

/** 面板本体：桌面圆角居中卡片（宽度由 className 给，如 sm:max-w-[560px]）；移动全宽底部抽屉。 */
function ModalDialog({ className, children }: ModalSectionProps) {
  return (
    <div
      className={cx(
        "pointer-events-auto relative flex w-full flex-col bg-overlay shadow-overlay outline-none",
        IS_MOBILE ? "max-h-[85vh] rounded-t-2xl pb-[env(safe-area-inset-bottom)]" : "max-h-full rounded-3xl p-6",
        className,
      )}
    >
      {children}
    </div>
  );
}

/* 移动端挂 Drawer 命名空间（语义同为 h2 标题/关闭按钮，aria 接线各自到最近的 Root）。 */
const TitlePart = (IS_MOBILE ? BaseDrawer.Title : BaseDialog.Title) as ComponentType<ModalSectionProps>;
const ClosePart = (IS_MOBILE ? BaseDrawer.Close : BaseDialog.Close) as ComponentType<
  ModalSectionProps & { "aria-label"?: string }
>;

function Header({ className, children }: ModalSectionProps) {
  return (
    <div
      className={cx(
        IS_MOBILE ? "relative flex items-center justify-center px-4 pt-4 pb-1" : "mb-4 flex items-center gap-3 pr-8",
        className,
      )}
    >
      {children}
    </div>
  );
}

function Heading({ className, children }: ModalSectionProps) {
  return (
    <TitlePart
      className={cx(
        IS_MOBILE ? "text-center text-base font-semibold" : "text-lg font-semibold text-foreground",
        className,
      )}
    >
      {children}
    </TitlePart>
  );
}

function Body({ className, children }: ModalSectionProps) {
  return (
    <div
      // 滚动区豁免 drawer 滑动手势，避免长表单滚动误触发关闭
      data-base-ui-swipe-ignore={IS_MOBILE ? "" : undefined}
      className={cx(
        IS_MOBILE
          ? "min-h-0 flex-1 overflow-y-auto overscroll-contain px-4 py-2 text-sm"
          : "min-h-0 text-sm leading-[1.43] text-muted",
        className,
      )}
    >
      {children}
    </div>
  );
}

function Footer({ className, children }: ModalSectionProps) {
  return <div className={cx(IS_MOBILE ? "px-4 pt-2 pb-2" : "mt-4 flex justify-end gap-2", className)}>{children}</div>;
}

function CloseTrigger({ className }: { className?: string }) {
  return (
    <ClosePart
      aria-label="关闭"
      className={cx(
        "absolute inline-flex size-8 cursor-pointer items-center justify-center rounded-full text-muted transition-colors",
        "outline-none hover:bg-default-soft hover:text-foreground focus-visible:outline-2 focus-visible:outline-focus",
        IS_MOBILE ? "top-2 right-2" : "top-4 right-4",
        className,
      )}
    >
      <XMarkIcon className="size-5" />
    </ClosePart>
  );
}

export const Modal = {
  Backdrop,
  Container,
  Dialog: ModalDialog,
  Header,
  Heading,
  Body,
  Footer,
  CloseTrigger,
};

/* -------------------------------------------------------------------------- */
/* BottomSheet：移动端存量便捷封装（对齐移动 ui.tsx 旧 API）                      */
/* -------------------------------------------------------------------------- */

export interface BottomSheetProps {
  opened: boolean;
  onClose: () => void;
  title?: ReactNode;
  footer?: ReactNode;
  className?: string;
  children?: ReactNode;
}

export function BottomSheet({ opened, onClose, title, footer, className, children }: BottomSheetProps) {
  return (
    <Backdrop isOpen={opened} onOpenChange={(open) => !open && onClose()}>
      <Container>
        <ModalDialog className={className}>
          {title != null && (
            <Header>
              <Heading>{title}</Heading>
              <CloseTrigger />
            </Header>
          )}
          <Body>{children}</Body>
          {footer != null && <Footer>{footer}</Footer>}
        </ModalDialog>
      </Container>
    </Backdrop>
  );
}
