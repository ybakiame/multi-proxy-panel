import { useState } from "react";
import { TextField, Label, TextArea, FieldError } from "@heroui/react";

interface JsonEditorProps {
  value: string;
  onChange: (value: string) => void;
  error?: string;
  label?: string;
  placeholder?: string;
}

export function JsonEditor({ value, onChange, error, label, placeholder }: JsonEditorProps) {
  const [isValid, setIsValid] = useState(true);

  const handleChange = (newValue: string) => {
    onChange(newValue);
    if (!newValue.trim()) {
      setIsValid(true);
      return;
    }
    try {
      JSON.parse(newValue);
      setIsValid(true);
    } catch {
      setIsValid(false);
    }
  };

  const invalid = !isValid || !!error;
  const errorMessage = error || (!isValid ? "Invalid JSON" : undefined);

  return (
    <TextField isInvalid={invalid}>
      {label && <Label>{label}</Label>}
      <TextArea
        value={value}
        onChange={(e) => handleChange(e.target.value)}
        placeholder={placeholder || "{}"}
        className="font-mono"
        rows={4}
      />
      {errorMessage && <FieldError>{errorMessage}</FieldError>}
    </TextField>
  );
}
