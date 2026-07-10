import { Select, Label, ListBox } from "@heroui/react";

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
  placeholder?: string;
  className?: string;
}

export function FormSelect({
  label,
  value,
  onChange,
  options,
  isRequired,
  placeholder,
  className,
}: FormSelectProps) {
  return (
    <Select
      value={value || null}
      onChange={(v) => onChange((v as string) || "")}
      isRequired={isRequired}
      className={className}
      placeholder={placeholder}
    >
      {label && <Label>{label}</Label>}
      <Select.Trigger>
        <Select.Value />
        <Select.Indicator />
      </Select.Trigger>
      <Select.Popover>
        <ListBox>
          {options.map((opt) => (
            <ListBox.Item key={opt.id} id={opt.id} textValue={opt.label}>
              {opt.label}
            </ListBox.Item>
          ))}
        </ListBox>
      </Select.Popover>
    </Select>
  );
}
