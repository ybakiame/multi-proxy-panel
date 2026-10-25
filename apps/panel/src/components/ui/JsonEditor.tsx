import { useId, useState } from "react";
import { Label } from "./label";
import { Textarea } from "./textarea";

interface JsonEditorProps {
  value: string;
  onChange: (value: string) => void;
  error?: string;
  label?: string;
  placeholder?: string;
}

export function JsonEditor({ value, onChange, error, label, placeholder }: JsonEditorProps) {
  const [isValid, setIsValid] = useState(true);
  const id = useId();
  const errorId = `${id}-error`;

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
    <div>
      {label && <Label htmlFor={id}>{label}</Label>}
      <Textarea
        id={id}
        value={value}
        onChange={(event) => handleChange(event.target.value)}
        placeholder={placeholder || "{}"}
        className="font-mono"
        rows={4}
        aria-invalid={invalid || undefined}
        aria-describedby={errorMessage ? errorId : undefined}
      />
      {errorMessage && (
        <p id={errorId} className="mt-1 text-sm text-destructive">
          {errorMessage}
        </p>
      )}
    </div>
  );
}
