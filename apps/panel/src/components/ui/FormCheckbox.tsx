import { useId } from "react";
import { Checkbox } from "./checkbox";
import { Label } from "./label";

interface FormCheckboxProps {
  isSelected?: boolean;
  isDisabled?: boolean;
  onChange?: (selected: boolean) => void;
  children?: React.ReactNode;
}

export function FormCheckbox({ isSelected, isDisabled, onChange, children }: FormCheckboxProps) {
  const id = useId();

  return (
    <div className="flex items-center gap-2">
      <Checkbox
        id={id}
        checked={isSelected}
        disabled={isDisabled}
        onCheckedChange={(checked) => onChange?.(checked === true)}
      />
      <Label htmlFor={id}>{children}</Label>
    </div>
  );
}
