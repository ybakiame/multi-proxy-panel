import { useId } from "react";
import { Input } from "./input";
import { Label } from "./label";

interface SearchInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

export function SearchInput({ value, onChange, placeholder }: SearchInputProps) {
  const id = useId();

  return (
    <div className="max-w-xs">
      <Label htmlFor={id} className="sr-only">
        {placeholder || "Search..."}
      </Label>
      <Input
        id={id}
        type="text"
        value={value}
        onChange={(event) => onChange(event.target.value)}
        placeholder={placeholder || "Search..."}
      />
    </div>
  );
}
