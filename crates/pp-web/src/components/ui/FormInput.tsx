import { TextField, Label, Input, FieldError } from "@heroui/react";

interface FormInputProps {
  label?: string;
  value?: string;
  onChange?: (value: string) => void;
  type?: string;
  placeholder?: string;
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
  isRequired,
  isInvalid,
  errorMessage,
  isReadOnly,
  className,
}: FormInputProps) {
  return (
    <TextField
      value={value}
      onChange={onChange}
      isRequired={isRequired}
      isInvalid={isInvalid}
      className={className}
    >
      {label && <Label>{label}</Label>}
      <Input type={type} placeholder={placeholder} readOnly={isReadOnly} />
      {errorMessage && <FieldError>{errorMessage}</FieldError>}
    </TextField>
  );
}
