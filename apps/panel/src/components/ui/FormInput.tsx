import { useId } from "react";
import { Input } from "./input";
import { Label } from "./label";

interface FormInputProps {
  label?: string;
  value?: string;
  onChange?: (value: string) => void;
  type?: string;
  placeholder?: string;
  description?: string;
  isRequired?: boolean;
  isInvalid?: boolean;
  errorMessage?: string;
  isReadOnly?: boolean;
  className?: string;
}

export function FormInput({
  label,
  value,
  onChange,
  type,
  placeholder,
  description,
  isRequired,
  isInvalid,
  errorMessage,
  isReadOnly,
  className,
}: FormInputProps) {
  const id = useId();
  const descriptionId = `${id}-description`;
  const errorId = `${id}-error`;
  const describedBy = [description ? descriptionId : null, errorMessage ? errorId : null]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={className}>
      {label && <Label htmlFor={id}>{label}</Label>}
      <Input
        id={id}
        type={type}
        value={value}
        onChange={(event) => onChange?.(event.target.value)}
        placeholder={placeholder}
        required={isRequired}
        readOnly={isReadOnly}
        aria-invalid={isInvalid || undefined}
        aria-describedby={describedBy || undefined}
      />
      {description && (
        <p id={descriptionId} className="mt-1 text-sm text-muted-foreground">
          {description}
        </p>
      )}
      {errorMessage && (
        <p id={errorId} className="mt-1 text-sm text-destructive">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
