/**
 * Select（T2）——下拉选择器（Base UI Select），目标桌面表单形态
 * （移动端后续以 action sheet 形态另行评估，M4 前移动页面继续用 Konsta）。
 *
 * 复合 API 对齐 HeroUI v3 桌面用法：
 *   <Select aria-label value onChange placeholder fullWidth>
 *     <Select.Trigger><Select.Value /><Select.Indicator /></Select.Trigger>
 *     <Select.Popover>
 *       <ListBox>
 *         <ListBox.Item id="x" textValue="X">X</ListBox.Item>
 *       </ListBox>
 *     </Select.Popover>
 *   </Select>
 * 样式规格镜像 @heroui/styles select.css（trigger=rounded-field/bg-field/shadow-field，
 * popover=min-w-anchor/bg-overlay/shadow-overlay）。
 *
 * 与 HeroUI 的差异：value 为 string（空值用 undefined），onChange 回传 string | undefined。
 */
import { Select as BaseSelect } from "@base-ui/react/select";
import { createContext, useContext, type ReactNode } from "react";
import { CheckIcon, ChevronUpDownIcon } from "../icons.tsx";
import { cx } from "../utils.ts";

interface SelectContextValue {
  placeholder?: string;
  ariaLabel?: string;
  fullWidth?: boolean;
}

const SelectContext = createContext<SelectContextValue>({});

export interface SelectRootProps {
  value?: string;
  /** 值变化回调（清空时回传 undefined，对齐 HeroUI onChange 语义）。 */
  onChange?: (value: string | undefined) => void;
  placeholder?: string;
  fullWidth?: boolean;
  isDisabled?: boolean;
  "aria-label"?: string;
  className?: string;
  children?: ReactNode;
}

function SelectRoot({
  value,
  onChange,
  placeholder,
  fullWidth = false,
  isDisabled = false,
  "aria-label": ariaLabel,
  children,
}: SelectRootProps) {
  return (
    <SelectContext.Provider value={{ placeholder, ariaLabel, fullWidth }}>
      <BaseSelect.Root
        value={value ?? null}
        onValueChange={(v: string | null) => onChange?.(v ?? undefined)}
        disabled={isDisabled}
      >
        {children}
      </BaseSelect.Root>
    </SelectContext.Provider>
  );
}

function Trigger({ className, children }: { className?: string; children?: ReactNode }) {
  const { ariaLabel, fullWidth } = useContext(SelectContext);
  return (
    <BaseSelect.Trigger
      aria-label={ariaLabel}
      className={cx(
        "flex min-h-9 cursor-pointer items-center justify-between gap-2 rounded-field border border-border bg-field px-3 py-2",
        "text-sm text-field-foreground shadow-field outline-none transition-colors",
        "hover:bg-field-hover hover:border-field-border-hover",
        "focus-visible:border-focus data-[placeholder]:text-muted data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50",
        fullWidth ? "w-full" : "min-w-40",
        className,
      )}
    >
      {children}
    </BaseSelect.Trigger>
  );
}

function Value({ className }: { className?: string }) {
  const { placeholder } = useContext(SelectContext);
  return <BaseSelect.Value placeholder={placeholder} className={cx("truncate", className)} />;
}

function Indicator({ className }: { className?: string }) {
  return (
    <BaseSelect.Icon className={cx("flex shrink-0 text-muted", className)}>
      <ChevronUpDownIcon className="size-4" />
    </BaseSelect.Icon>
  );
}

function Popover({ className, children }: { className?: string; children?: ReactNode }) {
  return (
    <BaseSelect.Portal>
      {/* alignItemWithTrigger=false：下拉展开在 trigger 下方而非覆盖对齐选中项 */}
      <BaseSelect.Positioner alignItemWithTrigger={false} side="bottom" sideOffset={4} className="z-[60]">
        <BaseSelect.Popup
          className={cx(
            "max-h-[var(--available-height)] min-w-[var(--anchor-width)] overflow-y-auto overscroll-contain rounded-xl border border-border bg-overlay p-1 text-sm shadow-overlay outline-none",
            "transition-[opacity,scale] duration-150 data-[starting-style]:scale-[0.98] data-[starting-style]:opacity-0 data-[ending-style]:opacity-0",
            className,
          )}
        >
          {children}
        </BaseSelect.Popup>
      </BaseSelect.Positioner>
    </BaseSelect.Portal>
  );
}

/* -------------------------------------------------------------------------- */
/* ListBox：选项列表（对齐 HeroUI ListBox / ListBox.Item 用法）                  */
/* -------------------------------------------------------------------------- */

function ListBoxRoot({ className, children }: { className?: string; children?: ReactNode }) {
  return <BaseSelect.List className={cx("outline-none", className)}>{children}</BaseSelect.List>;
}

export interface ListBoxItemProps {
  /** 选项值（HeroUI 的 id prop）。 */
  id: string;
  /** 供 typeahead 与 Value 显示的文本（HeroUI 的 textValue prop）。 */
  textValue?: string;
  isDisabled?: boolean;
  className?: string;
  children?: ReactNode;
}

function ListBoxItem({ id, textValue, isDisabled = false, className, children }: ListBoxItemProps) {
  return (
    <BaseSelect.Item
      value={id}
      label={textValue}
      disabled={isDisabled}
      className={cx(
        "flex cursor-default items-center justify-between gap-2 rounded-lg px-3 py-2 text-sm outline-none select-none",
        "data-[disabled]:cursor-not-allowed data-[disabled]:opacity-50 data-[highlighted]:bg-surface-secondary",
        className,
      )}
    >
      <BaseSelect.ItemText className="truncate">{children}</BaseSelect.ItemText>
      <BaseSelect.ItemIndicator className="flex shrink-0 text-accent">
        <CheckIcon className="size-4" />
      </BaseSelect.ItemIndicator>
    </BaseSelect.Item>
  );
}

/** 选项列表根（`<ListBox>` 元素）+ `ListBox.Item` 复合，对齐 HeroUI 用法。 */
export const ListBox = Object.assign(ListBoxRoot, { Item: ListBoxItem });

export const Select = Object.assign(SelectRoot, {
  Root: SelectRoot,
  Trigger,
  Value,
  Indicator,
  Popover,
  ListBox,
});
