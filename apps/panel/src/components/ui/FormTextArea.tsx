import { TextField, Label, TextArea, FieldError } from "@heroui/react";

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
  return (
    <TextField
      value={value}
      onChange={onChange}
      isRequired={isRequired}
      isInvalid={isInvalid}
      className={className}
    >
      {label && <Label>{label}</Label>}
      <TextArea placeholder={placeholder} rows={rows} />
      {errorMessage && <FieldError>{errorMessage}</FieldError>}
    </TextField>
  );
}
