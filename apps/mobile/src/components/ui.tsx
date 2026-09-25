/**
 * 移动端 UI 适配层（Konsta UI 迁移）。
 *
 * 收拢原 HeroUI 组件在本项目中的使用子集，以相近的 props 接口包裹 Konsta UI
 * 或纯 Tailwind 实现，将页面迁移成本收敛为「改 import + 改颜色类」：
 * - `Button`：variant(primary/secondary/tertiary/danger)、isPending、onPress → Konsta Button；
 * - `Card` 复合结构（Header/Title/Description/Content/Footer）→ 纯 Tailwind；
 * - `Chip`（soft 形态）、`Spinner`（→ Konsta Preloader）、`Switch`（→ Konsta Toggle）；
 * - `BottomSheet`：底部弹层通用骨架（标题 + 关闭 + 滚动内容区 + 底部动作区）；
 * - `ConfirmDialog`：确认框（→ Konsta Dialog + DialogButton）；
 * - `inputClassName`：原生 input 的统一样式（表单沿用原生 input）。
 */
import type { CSSProperties, ReactNode } from "react";
import {
  Button as KonstaButton,
  Card as KonstaCard,
  Dialog,
  DialogButton,
  Preloader,
  Sheet,
  Toggle,
} from "konsta/react";

// ---------- Button ----------

export type ButtonVariant = "primary" | "secondary" | "tertiary" | "danger";

interface ButtonProps {
  variant?: ButtonVariant;
  size?: "sm" | "md" | "lg";
  /** 禁用（等价原 HeroUI `isDisabled`）。 */
  isDisabled?: boolean;
  /** 进行中：按钮内嵌 spinner 并禁用（等价原 HeroUI `isPending`）。 */
  isPending?: boolean;
  /** 仅图标按钮（正方形、去水平内边距）。 */
  isIconOnly?: boolean;
  /** 点击回调（等价原 HeroUI `onPress`；内部转 Konsta onClick）。 */
  onPress?: () => void;
  type?: "button" | "submit";
  className?: string;
  style?: CSSProperties;
  "aria-label"?: string;
  children?: ReactNode;
}

/** 实心填充（primary/danger）时 spinner 跟随按钮文字白色，其余跟随主题主色。 */
const FILLED_VARIANTS: ReadonlySet<ButtonVariant> = new Set(["primary", "danger"]);

export function Button({
  variant = "primary",
  size = "md",
  isDisabled = false,
  isPending = false,
  isIconOnly = false,
  onPress,
  type = "button",
  className = "",
  style,
  children,
  ...rest
}: ButtonProps) {
  const tonal = variant === "secondary";
  const clear = variant === "tertiary";
  const colors =
    variant === "danger"
      ? { fillBgIos: "bg-red-500 active:bg-red-600" }
      : variant === "secondary"
        ? {
            tonalBgIos: "bg-black/5 active:bg-black/10 dark:bg-white/10 dark:active:bg-white/15",
            tonalTextIos: "text-zinc-900 dark:text-zinc-100",
          }
        : undefined;

  return (
    <KonstaButton
      // Konsta Button 默认 w-full（会撑爆 flex 区头）；inline 使宽度自适应内容，
      // 需要全宽/弹性宽度的调用点经 className（w-full / flex-1）显式声明。
      inline
      tonal={tonal}
      clear={clear}
      colors={colors}
      small={size === "sm"}
      large={size === "lg"}
      disabled={isDisabled || isPending}
      onClick={onPress}
      type={type}
      style={style}
      className={`${isIconOnly ? "aspect-square !px-0" : ""} ${className}`}
      {...rest}
    >
      {isPending ? (
        <span className="inline-flex items-center gap-2">
          <Spinner size="sm" color={FILLED_VARIANTS.has(variant) ? "white" : "accent"} />
          {isIconOnly ? null : children}
        </span>
      ) : (
        children
      )}
    </KonstaButton>
  );
}

// ---------- Card（复合结构，纯 Tailwind 实现） ----------

interface CardSectionProps {
  className?: string;
  children?: ReactNode;
}

function CardRoot({ className = "", children }: CardSectionProps) {
  // 卡片本体委托 Konsta Card（iOS 圆角浅色卡 / Material 海拔阴影随主题切换），
  // contentWrap 关闭：内边距由下方复合分区（Header/Content/Footer）自行控制。
  // 注意：Konsta Card 自带 `mx-safe-4` 外边距，twMerge 无法识别 safe 缩放、
  // 普通 `m-0` 覆盖不掉（会与容器 padding 叠加成双倍边距），必须用 important。
  return (
    <KonstaCard contentWrap={false} className={`m-0! ${className}`}>
      {children}
    </KonstaCard>
  );
}

function CardHeader({ className = "", children }: CardSectionProps) {
  return <div className={`flex flex-col gap-0.5 px-4 pt-4 ${className}`}>{children}</div>;
}

function CardTitle({ className = "", children }: CardSectionProps) {
  return <h3 className={`text-base font-semibold text-zinc-900 dark:text-zinc-100 ${className}`}>{children}</h3>;
}

function CardDescription({ className = "", children }: CardSectionProps) {
  return <p className={`text-sm text-zinc-500 dark:text-zinc-400 ${className}`}>{children}</p>;
}

function CardContent({ className = "", children }: CardSectionProps) {
  return <div className={`p-4 ${className}`}>{children}</div>;
}

function CardFooter({ className = "", children }: CardSectionProps) {
  return <div className={`px-4 pb-4 ${className}`}>{children}</div>;
}

/** 卡片容器（对齐原 HeroUI `Card` 复合用法；本体为 Konsta `Card`）。 */
export const Card = Object.assign(CardRoot, {
  Header: CardHeader,
  Title: CardTitle,
  Description: CardDescription,
  Content: CardContent,
  Footer: CardFooter,
});

// ---------- Chip（soft 形态徽标） ----------

export type ChipColor = "default" | "accent" | "danger" | "warning" | "success";

const CHIP_COLORS: Record<ChipColor, string> = {
  default: "bg-zinc-500/15 text-zinc-600 dark:text-zinc-300",
  accent: "bg-primary/15 text-primary",
  danger: "bg-red-500/15 text-red-600 dark:text-red-400",
  warning: "bg-amber-500/15 text-amber-700 dark:text-amber-400",
  success: "bg-green-500/15 text-green-700 dark:text-green-400",
};

interface ChipProps {
  color?: ChipColor;
  className?: string;
  children?: ReactNode;
}

/** 软底色徽标（对齐原 HeroUI `Chip` 的 `variant="soft" size="sm"` 用法）。 */
export function Chip({ color = "default", className = "", children }: ChipProps) {
  return (
    <span
      className={`inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium ${CHIP_COLORS[color]} ${className}`}
    >
      {children}
    </span>
  );
}

// ---------- Spinner ----------

interface SpinnerProps {
  size?: "sm" | "md";
  /** `accent` = 主题主色（默认）；`current`/`white` = 跟随/固定文字色。 */
  color?: "accent" | "current" | "white";
  className?: string;
  "aria-hidden"?: boolean | "true" | "false";
}

/** 加载指示（原 HeroUI `Spinner` → Konsta `Preloader`）。 */
export function Spinner({ size = "md", color = "accent", className = "", ...rest }: SpinnerProps) {
  const iconIos = color === "current" ? "text-current" : color === "white" ? "text-white" : undefined;
  return (
    <Preloader
      colors={iconIos ? { iconIos } : undefined}
      className={`${size === "sm" ? "size-5" : "size-7"} ${className}`}
      {...rest}
    />
  );
}

// ---------- Switch ----------

interface SwitchProps {
  /** 选中态（等价原 HeroUI `isSelected`）。 */
  isSelected?: boolean;
  /** 变更回调（等价原 HeroUI `onValueChange`）。 */
  onValueChange?: (value: boolean) => void;
  isDisabled?: boolean;
  "aria-label"?: string;
}

/** 开关（原 HeroUI `Switch` → Konsta `Toggle`）。 */
export function Switch({ isSelected = false, onValueChange, isDisabled = false, ...rest }: SwitchProps) {
  return (
    <Toggle
      checked={isSelected}
      disabled={isDisabled}
      onChange={(e: { target: { checked: boolean } }) => onValueChange?.(e.target.checked)}
      {...rest}
    />
  );
}

// ---------- BottomSheet（底部弹层通用骨架） ----------

interface BottomSheetProps {
  opened: boolean;
  onClose: () => void;
  /** 标题（缺省不渲染头部）。 */
  title?: ReactNode;
  /** 底部动作区（如保存按钮），固定在内容区下方。 */
  footer?: ReactNode;
  children?: ReactNode;
}

/**
 * 底部弹层骨架（替代原 HeroUI `Modal.Backdrop/Container placement="bottom"` 复合结构）。
 *
 * - Konsta `Sheet`：点击背板关闭；内容区 `max-h-[70vh]` 滚动；
 * - 头部：标题 + 右侧关闭按钮；底部 `env(safe-area-inset-bottom)` 适配系统手势条。
 */
export function BottomSheet({ opened, onClose, title, footer, children }: BottomSheetProps) {
  return (
    <Sheet
      opened={opened}
      onBackdropClick={onClose}
      className="flex max-h-[85vh] flex-col pb-[env(safe-area-inset-bottom)]"
    >
      {title !== undefined && (
        <div className="relative flex items-center justify-center px-4 pb-2 pt-4">
          <h2 className="truncate px-10 text-center text-base font-semibold">{title}</h2>
          <button
            type="button"
            aria-label="关闭"
            onClick={onClose}
            className="absolute right-2 top-1/2 flex size-9 -translate-y-1/2 items-center justify-center rounded-full text-zinc-500 active:bg-black/5 dark:text-zinc-400 dark:active:bg-white/10"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} className="size-5">
              <path strokeLinecap="round" d="M6 6l12 12M18 6L6 18" />
            </svg>
          </button>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-2">{children}</div>
      {footer ? <div className="px-4 pb-2 pt-2">{footer}</div> : null}
    </Sheet>
  );
}

// ---------- ConfirmDialog（确认框） ----------

interface ConfirmDialogProps {
  opened: boolean;
  /** 标题（如「删除订阅」）。 */
  title: ReactNode;
  /** 正文内容。 */
  children?: ReactNode;
  confirmText?: string;
  cancelText?: string;
  /** 危险操作（确认按钮红色文案）。 */
  danger?: boolean;
  /** 确认进行中（确认按钮禁用；Konsta DialogButton 无内嵌 spinner，以禁用态呈现）。 */
  busy?: boolean;
  onConfirm: () => void;
  onClose: () => void;
}

/** 确认对话框（替代原 HeroUI `AlertDialog` 复合结构 → Konsta `Dialog`）。 */
export function ConfirmDialog({
  opened,
  title,
  children,
  confirmText = "确认",
  cancelText = "取消",
  danger = false,
  busy = false,
  onConfirm,
  onClose,
}: ConfirmDialogProps) {
  return (
    <Dialog
      opened={opened}
      onBackdropClick={() => {
        if (!busy) onClose();
      }}
      title={title}
      content={<div className="text-left">{children}</div>}
      buttons={
        <>
          <DialogButton onClick={onClose} disabled={busy}>
            {cancelText}
          </DialogButton>
          <DialogButton strong onClick={onConfirm} disabled={busy} className={danger ? "!text-red-500" : undefined}>
            {busy ? "处理中…" : confirmText}
          </DialogButton>
        </>
      }
    />
  );
}

// ---------- 表单输入 ----------

/** 原生 input/textarea 的统一样式（Konsta 迁移后表单沿用原生 input）。 */
export const inputClassName =
  "h-12 w-full rounded-lg border border-zinc-300 bg-white px-3 text-sm text-zinc-900 outline-none placeholder:text-zinc-400 focus:border-primary/60 disabled:opacity-60 dark:border-zinc-700 dark:bg-zinc-800 dark:text-zinc-100 dark:placeholder:text-zinc-500";

/** 多行输入（textarea）变体：去固定行高、加纵向内边距。 */
export const textareaClassName = `${inputClassName} h-auto min-h-24 py-2`;
