import { useId } from "react";
import { Label } from "./label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./select";

interface FormSelectOption {
  id: string;
  label: string;
}

interface FormSelectProps {
  label?: string;
  value: string;
  onChange: (value: string) => void;
  options: FormSelectOption[];
  isRequired?: boolean;
  isDisabled?: boolean;
  placeholder?: string;
  description?: string;
  className?: string;
}

export function FormSelect({
  label,
  value,
  onChange,
  options,
  isRequired,
  isDisabled,
  placeholder,
  description,
  className,
}: FormSelectProps) {
  const id = useId();

  return (
    <div className={className}>
      {label && <Label htmlFor={id}>{label}</Label>}
      <Select
        value={value || null}
        onValueChange={(nextValue) => onChange(nextValue || "")}
        required={isRequired}
        disabled={isDisabled}
      >
        <SelectTrigger
          id={id}
          className="w-full"
          aria-required={isRequired || undefined}
          aria-describedby={description ? `${id}-description` : undefined}
        >
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          {options.map((opt) => (
            <SelectItem key={opt.id} value={opt.id}>
              {opt.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {description && (
        <p id={`${id}-description`} className="mt-1 text-sm text-muted-foreground">
          {description}
        </p>
      )}
    </div>
  );
}
