import { Input, Label, ListBox, Select, Switch } from "@heroui/react";

/**
 * 自定义出站表单基础控件（桌面端；语义对齐移动端 `Outbounds/OutboundField.tsx`，
 * HeroUI Input/Select/Switch 实现）。
 */

interface TextFieldProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** 行内错误（优先于 hint 展示）。 */
  error?: string | null;
  hint?: string;
  inputMode?: "text" | "numeric";
  mono?: boolean;
  required?: boolean;
}

/** 带标签 / 错误 / 提示的文本输入行。 */
export function TextField({
  id,
  label,
  value,
  onChange,
  placeholder,
  error,
  hint,
  inputMode,
  mono = false,
  required = false,
}: TextFieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      <Input
        id={id}
        aria-label={label}
        aria-required={required || undefined}
        inputMode={inputMode}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder}
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        className={mono ? "font-mono" : undefined}
      />
      {error ? (
        <span className="text-xs text-amber-500">{error}</span>
      ) : hint ? (
        <span className="text-xs text-muted">{hint}</span>
      ) : null}
    </div>
  );
}

interface SelectFieldProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
  options: { value: string; label: string }[];
  placeholder?: string;
  /** 行内错误（优先于 hint 展示）。 */
  error?: string | null;
  hint?: string;
}

/** 带标签 / 错误 / 提示的下拉选择行。 */
export function SelectField({ label, value, onChange, options, placeholder, error, hint }: SelectFieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <Label>{label}</Label>
      <Select
        aria-label={label}
        value={value}
        onChange={(key) => onChange(String(key ?? ""))}
        placeholder={placeholder}
        fullWidth
      >
        <Select.Trigger>
          <Select.Value />
          <Select.Indicator />
        </Select.Trigger>
        <Select.Popover>
          <ListBox>
            {options.map((option) => (
              <ListBox.Item key={option.value} id={option.value} textValue={option.label}>
                {option.label}
                <ListBox.ItemIndicator />
              </ListBox.Item>
            ))}
          </ListBox>
        </Select.Popover>
      </Select>
      {error ? (
        <span className="text-xs text-amber-500">{error}</span>
      ) : hint ? (
        <span className="text-xs text-muted">{hint}</span>
      ) : null}
    </div>
  );
}

interface SwitchRowProps {
  label: string;
  ariaLabel: string;
  isSelected: boolean;
  onChange: (next: boolean) => void;
}

/** 左标签 + 右开关的整行开关。 */
export function SwitchRow({ label, ariaLabel, isSelected, onChange }: SwitchRowProps) {
  return (
    <div className="flex items-center justify-between gap-3">
      <span className="text-sm text-foreground">{label}</span>
      <Switch aria-label={ariaLabel} isSelected={isSelected} onChange={onChange}>
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Content>
      </Switch>
    </div>
  );
}

/** 嵌套区块分区标题（TLS / 传输方式）。 */
export function SectionTitle({ children }: { children: string }) {
  return <span className="pt-1 text-xs font-semibold uppercase tracking-wide text-muted">{children}</span>;
}
