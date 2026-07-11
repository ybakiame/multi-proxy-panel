import { useMemo } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { oneDark } from "@codemirror/theme-one-dark";

export type CodeLanguage = "json" | "yaml" | "text";

interface CodeEditorProps {
  value: string;
  onChange: (value: string) => void;
  language?: CodeLanguage;
  label?: string;
  placeholder?: string;
  height?: string;
  className?: string;
}

export function CodeEditor({
  value,
  onChange,
  language = "text",
  label,
  placeholder,
  height = "240px",
  className = "",
}: CodeEditorProps) {
  const extensions = useMemo(() => {
    switch (language) {
      case "json":
        return [json()];
      case "yaml":
        return [yaml()];
      default:
        return [];
    }
  }, [language]);

  return (
    <div className={`space-y-1 ${className}`}>
      {label && <label className="block text-sm font-medium text-foreground">{label}</label>}
      <CodeMirror
        value={value}
        height={height}
        extensions={extensions}
        theme={oneDark}
        placeholder={placeholder}
        onChange={onChange}
        className="rounded-md border border-divider overflow-hidden text-sm"
        basicSetup={{
          lineNumbers: true,
          highlightActiveLineGutter: true,
          highlightActiveLine: true,
          foldGutter: true,
        }}
      />
    </div>
  );
}
