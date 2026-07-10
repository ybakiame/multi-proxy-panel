import { TextField, Label, Input } from "@heroui/react";

interface SearchInputProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
}

export function SearchInput({ value, onChange, placeholder }: SearchInputProps) {
  return (
    <TextField value={value} onChange={onChange} className="max-w-xs">
      <Label className="sr-only">{placeholder || "Search..."}</Label>
      <Input type="text" placeholder={placeholder || "Search..."} />
    </TextField>
  );
}
