import { useMemo } from "react";
import CodeMirror from "@uiw/react-codemirror";
import { json } from "@codemirror/lang-json";
import { yaml } from "@codemirror/lang-yaml";
import { oneDark } from "@codemirror/theme-one-dark";
import { EditorView } from "@codemirror/view";

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
  height = "320px",
  className = "",
}: CodeEditorProps) {
  const extensions = useMemo(() => {
    const base = [EditorView.lineWrapping];
    switch (language) {
      case "json":
        return [...base, json()];
      case "yaml":
        return [...base, yaml()];
      default:
        return base;
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
        className="rounded-md border border-divider overflow-hidden text-sm font-mono"
        basicSetup={{
          lineNumbers: true,
          highlightActiveLineGutter: true,
          highlightActiveLine: true,
          foldGutter: true,
          bracketMatching: true,
          closeBrackets: true,
          autocompletion: true,
          tabSize: 2,
        }}
      />
    </div>
  );
}
