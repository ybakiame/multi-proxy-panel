import { Checkbox } from "@heroui/react";

interface FormCheckboxProps {
  isSelected?: boolean;
  isDisabled?: boolean;
  onChange?: (selected: boolean) => void;
  children?: React.ReactNode;
}

export function FormCheckbox({ isSelected, isDisabled, onChange, children }: FormCheckboxProps) {
  return (
    <Checkbox isSelected={isSelected} isDisabled={isDisabled} onChange={onChange}>
      <Checkbox.Content>
        <Checkbox.Control>
          <Checkbox.Indicator />
        </Checkbox.Control>
        {children}
      </Checkbox.Content>
    </Checkbox>
  );
}
