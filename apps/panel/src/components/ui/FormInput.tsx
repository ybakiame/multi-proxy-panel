import { TextField, Label, Input, FieldError, Description } from "@heroui/react";

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
      {description && <Description>{description}</Description>}
      {errorMessage && <FieldError>{errorMessage}</FieldError>}
    </TextField>
  );
}
