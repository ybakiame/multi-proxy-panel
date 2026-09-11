import { Switch } from "@heroui/react";
import { MobileSelectSheet } from "../../../components/MobileSelectSheet";

/**
 * 自定义出站表单基础控件（ADR-0005 P0-4c）。
 *
 * 统一文本输入 / 选择器 / 开关行 / 分区标题的样式与错误提示布局，供
 * `OutboundProtocolFields` / `OutboundTlsFields` / `OutboundTransportFields` 复用，
 * 避免逐字段重复（输入框样式对齐 `DnsServerFormSheet` 的 `inputClass`）。
 */

/** 文本输入样式（对齐 DnsServerFormSheet 的 inputClass）。 */
export const OUTBOUND_INPUT_CLASS =
  "h-12 w-full rounded-lg border border-border/70 bg-surface px-3 text-sm text-foreground outline-none " +
  "placeholder:text-muted focus:border-accent/60 disabled:opacity-60";

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
      <label htmlFor={id} className="text-sm font-medium text-foreground">
        {label}
      </label>
      <input
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
        className={`${OUTBOUND_INPUT_CLASS}${mono ? " font-mono" : ""}`}
      />
      {error ? (
        <span className="text-xs text-warning">{error}</span>
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

/** 带标签 / 错误 / 提示的底部弹层选择行。 */
export function SelectField({ label, value, onChange, options, placeholder, error, hint }: SelectFieldProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="text-sm font-medium text-foreground">{label}</span>
      <MobileSelectSheet label={label} value={value} onChange={onChange} options={options} placeholder={placeholder} />
      {error ? (
        <span className="text-xs text-warning">{error}</span>
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

/** 左标签 + 右开关的整行开关（触达区 ≥44px）。 */
export function SwitchRow({ label, ariaLabel, isSelected, onChange }: SwitchRowProps) {
  return (
    <div className="flex min-h-11 items-center justify-between gap-3">
      <span className="text-sm text-foreground">{label}</span>
      <Switch aria-label={ariaLabel} isSelected={isSelected} onChange={onChange} className="shrink-0">
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
