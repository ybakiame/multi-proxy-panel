import { useState } from "react";
import { CheckIcon, ChevronDownIcon } from "@heroicons/react/24/outline";
import { Modal } from "@heroui/react";

/** 移动选择器选项。 */
export interface MobileSelectOption {
  /** 写入回调的值。 */
  value: string;
  /** 触发器和列表行显示名。 */
  label: string;
  /** 可选说明行（列表行副标题，如不可用原值提示）。 */
  description?: string;
}

interface MobileSelectSheetProps {
  /** 弹层标题，同时作为触发器的可访问名。 */
  label: string;
  options: MobileSelectOption[];
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
 * - 点击打开 `Modal` 底部弹层（`max-h-[62vh]` 滚动），列表行 `min-h-12`，
 *   选中态高亮 + CheckIcon，点击行选中并关闭；
 * - 无选项或 `disabled` 时触发器禁用；空选项弹层内展示空态；
 * - 可访问性：触发器 `aria-haspopup="dialog"`/`aria-expanded`，列表行按钮
 *   `aria-pressed` 标记选中态。
 */
export function MobileSelectSheet({
  label,
  options,
  value,
  onChange,
  placeholder = "请选择",
  disabled = false,
}: MobileSelectSheetProps) {
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
        className="flex min-h-12 w-full items-center justify-between gap-2 rounded-lg border border-border/70 bg-surface px-3 text-left text-sm outline-none transition-colors active:bg-surface-secondary/60 disabled:opacity-60"
      >
        <span className={`min-w-0 truncate ${selected ? "text-foreground" : "text-muted"}`}>
          {selected ? selected.label : placeholder}
        </span>
        <ChevronDownIcon className="size-5 shrink-0 text-muted" aria-hidden="true" />
      </button>

      <Modal.Backdrop isOpen={open} onOpenChange={setOpen} isDismissable>
        <Modal.Container placement="bottom">
          <Modal.Dialog>
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>{label}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex max-h-[62vh] flex-col gap-1.5 overflow-y-auto">
              {isEmpty ? (
                <div className="flex flex-col gap-1 py-8 text-center">
                  <span className="text-sm text-muted">暂无可选项</span>
                </div>
              ) : (
                <div className="flex flex-col gap-1.5">
                  {options.map((opt) => {
                    const isSelected = opt.value === value;
                    return (
                      <button
                        key={opt.value}
                        type="button"
                        aria-pressed={isSelected}
                        onClick={() => handleSelect(opt.value)}
                        className={`flex min-h-12 w-full items-center justify-between gap-3 rounded-xl border px-4 py-2.5 text-left transition-colors ${
                          isSelected
                            ? "border-accent/50 bg-accent/5"
                            : "border-border/70 active:bg-surface-secondary/60"
                        }`}
                      >
                        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                          <span className="truncate text-sm font-medium text-foreground">{opt.label}</span>
                          {opt.description && <span className="truncate text-xs text-muted">{opt.description}</span>}
                        </span>
                        {isSelected && <CheckIcon className="size-5 shrink-0 text-accent" aria-hidden="true" />}
                      </button>
                    );
                  })}
                </div>
              )}
            </Modal.Body>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </>
  );
}
