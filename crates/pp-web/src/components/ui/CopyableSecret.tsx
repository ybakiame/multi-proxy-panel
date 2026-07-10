import { useState } from "react";
import { Button, TextField, Label, Input } from "@heroui/react";

interface CopyableSecretProps {
  secret: string;
  label?: string;
}

export function CopyableSecret({ secret, label }: CopyableSecretProps) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(secret);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="rounded-lg border border-warning/30 bg-warning/10 p-4">
      {label && <p className="text-sm font-medium text-warning">{label}</p>}
      <div className="mt-2 flex items-center gap-2">
        <TextField className="flex-1 font-mono">
          <Label className="sr-only">{label || "Secret"}</Label>
          <Input readOnly value={secret} />
        </TextField>
        <Button
          variant="secondary"
          size="sm"
          onPress={copy}
          className="text-warning"
        >
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}
