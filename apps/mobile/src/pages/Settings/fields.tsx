import { Switch } from "@heroui/react";

/** 移动设置页输入框统一样式（对齐 SubscriptionFormSheet 的输入外观）。 */
export const settingsInputClass =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 font-mono text-sm text-foreground outline-none placeholder:text-muted focus:border-primary/60 disabled:opacity-60";

export const settingsLabelClass = "text-sm font-medium text-foreground";

interface SwitchRowProps {
  label: string;
  description?: string;
  selected: boolean;
  disabled: boolean;
  onChange: (next: boolean) => void;
}

/** 左文右 Switch 的设置行（移动单列触达布局）。 */
export function SwitchRow({ label, description, selected, disabled, onChange }: SwitchRowProps) {
  return (
    <div className="flex items-center justify-between gap-3">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className={settingsLabelClass}>{label}</span>
        {description && <span className="text-xs text-muted">{description}</span>}
      </div>
      <Switch
        aria-label={label}
        isSelected={selected}
        isDisabled={disabled}
        onChange={(next) => onChange(next)}
        className="shrink-0"
      >
        <Switch.Content>
          <Switch.Control>
            <Switch.Thumb />
          </Switch.Control>
        </Switch.Content>
      </Switch>
    </div>
  );
}

interface SettingsInputProps {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  type?: "text" | "password";
  /** 移动键盘类型：numeric 弹数字键盘，url/text 按需。 */
  inputMode?: "text" | "numeric" | "url";
  /** 非法/未填时的行内错误（替代 hint 展示）。 */
  error?: string | null;
  /** 输入框下方的说明文案。 */
  hint?: string;
}

/** 带 Label 的设置输入项（错误优先于 hint 展示）。 */
export function SettingsInput({
  id,
  label,
  value,
  onChange,
  placeholder,
  disabled,
  type = "text",
  inputMode = "text",
  error,
  hint,
}: SettingsInputProps) {
  return (
    <div className="flex flex-col gap-2">
      <label htmlFor={id} className={settingsLabelClass}>
        {label}
      </label>
      <input
        id={id}
        aria-label={label}
        type={type}
        inputMode={inputMode}
        autoComplete="off"
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        value={value}
        placeholder={placeholder}
        disabled={disabled}
        onChange={(event) => onChange(event.target.value)}
        className={settingsInputClass}
      />
      {error ? (
        <span className="text-xs text-warning">{error}</span>
      ) : hint ? (
        <span className="text-xs text-muted">{hint}</span>
      ) : null}
    </div>
  );
}
