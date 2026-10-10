import { useState } from "react";
import { CheckIcon, ChevronUpDownIcon as ChevronDownIcon } from "../icons";
import { IS_MOBILE } from "../platform";
import { Select, ListBox } from "./Select";
import { BottomSheet } from "./Modal";

/** 移动选择器选项。 */
export interface SelectOption {
  /** 写入回调的值。 */
  value: string;
  /** 触发器和列表行显示名。 */
  label: string;
  /** 可选说明行（列表行副标题，如不可用原值提示）。 */
  description?: string;
}

interface SelectFieldProps {
  /** 弹层标题，同时作为触发器的可访问名。 */
  label: string;
  options: SelectOption[];
  value: string;
  onChange: (value: string) => void;
  /** 未选中时触发器占位文案。 */
  placeholder?: string;
  /** 外部禁用（如保存中）；与「无选项」共用禁用态。 */
  disabled?: boolean;
}

/**
 * 移动底部弹层选择器（移动形态替代桌面下拉 `Select`，ADR-0003 M5）。
 *
 * - 触发器：对齐输入框行高（`min-h-12`），显示当前选中 label + chevron-down；
 * - 点击打开 `BottomSheet` 底部弹层（`max-h-[62vh]` 滚动），列表行 `min-h-12`，
 *   选中态高亮 + CheckIcon，点击行选中并关闭；
 * - 无选项或 `disabled` 时触发器禁用；空选项弹层内展示空态；
 * - 可访问性：触发器 `aria-haspopup="dialog"`/`aria-expanded`，列表行按钮
 *   `aria-pressed` 标记选中态。
 */
function SheetPicker({ label, options, value, onChange, placeholder = "请选择", disabled = false }: SelectFieldProps) {
  const [open, setOpen] = useState(false);
  const selected = options.find((opt) => opt.value === value);
  const isEmpty = options.length === 0;

  const handleSelect = (next: string) => {
    onChange(next);
    setOpen(false);
  };

  return (
    <>
      <button
        type="button"
        aria-label={label}
        aria-haspopup="dialog"
        aria-expanded={open}
        disabled={disabled || isEmpty}
        onClick={() => setOpen(true)}
        className="flex min-h-12 w-full items-center justify-between gap-2 rounded-lg border border-zinc-300 bg-white px-3 text-left text-sm outline-none transition-colors active:bg-zinc-100 disabled:opacity-60 dark:border-zinc-700 dark:bg-zinc-800 dark:active:bg-zinc-700"
      >
        <span
          className={`min-w-0 truncate ${selected ? "text-zinc-900 dark:text-zinc-100" : "text-zinc-400 dark:text-zinc-500"}`}
        >
          {selected ? selected.label : placeholder}
        </span>
        <ChevronDownIcon className="size-5 shrink-0 text-zinc-400" aria-hidden="true" />
      </button>

      <BottomSheet opened={open} onClose={() => setOpen(false)} title={label}>
        <div className="flex max-h-[62vh] flex-col gap-1.5 overflow-y-auto">
          {isEmpty ? (
            <div className="flex flex-col gap-1 py-8 text-center">
              <span className="text-sm text-zinc-500 dark:text-zinc-400">暂无可选项</span>
            </div>
          ) : (
            options.map((opt) => {
              const isSelected = opt.value === value;
              return (
                <button
                  key={opt.value}
                  type="button"
                  aria-pressed={isSelected}
                  onClick={() => handleSelect(opt.value)}
                  className={`flex min-h-12 w-full items-center justify-between gap-3 rounded-xl border px-4 py-2.5 text-left transition-colors ${
                    isSelected
                      ? "border-primary/50 bg-primary/5"
                      : "border-zinc-200 active:bg-zinc-100 dark:border-zinc-700 dark:active:bg-zinc-800"
                  }`}
                >
                  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">{opt.label}</span>
                    {opt.description && (
                      <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{opt.description}</span>
                    )}
                  </span>
                  {isSelected && <CheckIcon className="size-5 shrink-0 text-primary" aria-hidden="true" />}
                </button>
              );
            })
          )}
        </div>
      </BottomSheet>
    </>
  );
}

/** 同一选项契约：桌面 dropdown，移动 Drawer picker。 */
export function SelectField(props: SelectFieldProps) {
  if (IS_MOBILE) return <SheetPicker {...props} />;
  return (
    <Select
      aria-label={props.label}
      value={props.options.some((option) => option.value === props.value) ? props.value : undefined}
      onChange={(value) => props.onChange(value ?? "")}
      placeholder={props.placeholder ?? "请选择"}
      isDisabled={props.disabled || props.options.length === 0}
      fullWidth
    >
      <Select.Trigger>
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {props.options.map((option) => (
            <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
              <span className="flex flex-col">
                <span>{option.label}</span>
                {option.description && <span className="text-xs text-muted">{option.description}</span>}
              </span>
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}
