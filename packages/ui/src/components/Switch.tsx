/**
 * Switch（T1，Base UI Switch 封装）。
 * 复合结构对齐桌面 HeroUI 用法：
 * ```tsx
 * <Switch isSelected onChange isDisabled>
 *   <Switch.Content>
 *     <Switch.Control><Switch.Thumb /></Switch.Control>
 *     标签文本
 *   </Switch.Content>
 * </Switch>
 * ```
 * 根组件收 `isSelected`/`onChange`/`isDisabled`（HeroUI 风格），经 context
 * 传给 `Control`（Base UI `Switch.Root`，`checked`/`onCheckedChange`）。
 */
import { Switch as BaseSwitch } from "@base-ui/react/switch";
import { createContext, useContext, type ReactNode } from "react";
import { IS_MOBILE } from "../platform.ts";
import { cx } from "../utils.ts";

interface SwitchContextValue {
  checked: boolean;
  disabled: boolean;
  onChange?: (value: boolean) => void;
  ariaLabel?: string;
}

const SwitchContext = createContext<SwitchContextValue>({ checked: false, disabled: false });

interface SwitchRootProps {
  size?: "sm" | "md" | "lg";
  "aria-label"?: string;
  onValueChange?: (value: boolean) => void;
  /** 选中态（等价 HeroUI `isSelected`）。 */
  isSelected?: boolean;
  /** 变更回调（等价 HeroUI `onChange`，参数为新布尔值）。 */
  onChange?: (value: boolean) => void;
  isDisabled?: boolean;
  className?: string;
  children?: ReactNode;
}

function SwitchRoot({
  isSelected = false,
  onChange,
  onValueChange,
  isDisabled = false,
  className,
  children,
  "aria-label": ariaLabel,
}: SwitchRootProps) {
  return (
    <SwitchContext.Provider
      value={{ checked: isSelected, disabled: isDisabled, onChange: onChange ?? onValueChange, ariaLabel }}
    >
      <span className={cx("inline-flex items-center", isDisabled && "cursor-not-allowed opacity-50", className)}>
        {children ?? (
          <SwitchControl>
            <SwitchThumb />
          </SwitchControl>
        )}
      </span>
    </SwitchContext.Provider>
  );
}

interface SwitchSectionProps {
  className?: string;
  children?: ReactNode;
}

function SwitchContent({ className, children }: SwitchSectionProps) {
  return <label className={cx("flex items-center gap-2 text-sm text-foreground", className)}>{children}</label>;
}

function SwitchControl({ className, children }: SwitchSectionProps) {
  const { checked, disabled, onChange, ariaLabel } = useContext(SwitchContext);
  return (
    <BaseSwitch.Root
      aria-label={ariaLabel}
      checked={checked}
      disabled={disabled}
      onCheckedChange={(next) => onChange?.(next)}
      className={cx(
        "relative inline-flex shrink-0 cursor-pointer items-center rounded-full bg-default transition-colors",
        "outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus",
        "data-checked:bg-accent data-disabled:cursor-not-allowed",
        IS_MOBILE ? "h-8 w-[3.25rem]" : "h-6 w-11",
        className,
      )}
    >
      {children}
    </BaseSwitch.Root>
  );
}

function SwitchThumb({ className }: { className?: string }) {
  return (
    <BaseSwitch.Thumb
      className={cx(
        "block translate-x-0.5 rounded-full bg-white shadow transition-transform",
        IS_MOBILE ? "size-7 data-checked:translate-x-[1.375rem]" : "size-5 data-checked:translate-x-[1.375rem]",
        className,
      )}
    />
  );
}

export const Switch = Object.assign(SwitchRoot, {
  Content: SwitchContent,
  Control: SwitchControl,
  Thumb: SwitchThumb,
});
