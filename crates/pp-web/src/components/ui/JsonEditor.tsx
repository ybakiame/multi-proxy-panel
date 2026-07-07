import { useState } from "react";
import { Textarea } from "@heroui/react";

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

  return (
    <div className="space-y-1">
      {label && <label className="text-sm font-medium">{label}</label>}
      <Textarea
        value={value}
        onChange={(e) => handleChange(e.target.value)}
        placeholder={placeholder || "{}"}
        isInvalid={!isValid || !!error}
        errorMessage={error || (!isValid ? "Invalid JSON" : undefined)}
        className="font-mono"
        minRows={4}
      />
    </div>
  );
}
