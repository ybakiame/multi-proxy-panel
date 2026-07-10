import { Checkbox } from "@heroui/react";

interface FormCheckboxProps {
  isSelected?: boolean;
  onChange?: (selected: boolean) => void;
  children?: React.ReactNode;
}

export function FormCheckbox({ isSelected, onChange, children }: FormCheckboxProps) {
  return (
    <Checkbox isSelected={isSelected} onChange={onChange}>
      <Checkbox.Content>
        <Checkbox.Control>
          <Checkbox.Indicator />
        </Checkbox.Control>
        {children}
      </Checkbox.Content>
    </Checkbox>
  );
}
