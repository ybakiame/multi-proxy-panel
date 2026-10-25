import { useId } from "react";
import { Label } from "./label";
import { Textarea } from "./textarea";

interface FormTextAreaProps {
  label?: string;
  value?: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  isRequired?: boolean;
  isInvalid?: boolean;
  errorMessage?: string;
  className?: string;
  rows?: number;
}

export function FormTextArea({
  label,
  value,
  onChange,
  placeholder,
  isRequired,
  isInvalid,
  errorMessage,
  className,
  rows,
}: FormTextAreaProps) {
  const id = useId();
  const errorId = `${id}-error`;

  return (
    <div className={className}>
      {label && <Label htmlFor={id}>{label}</Label>}
      <Textarea
        id={id}
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
        placeholder={placeholder}
        required={isRequired}
        rows={rows}
        aria-invalid={isInvalid || undefined}
        aria-describedby={errorMessage ? errorId : undefined}
      />
      {errorMessage && (
        <p id={errorId} className="mt-1 text-sm text-destructive">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
