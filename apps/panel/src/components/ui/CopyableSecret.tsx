import { useId, useState } from "react";
import { Button } from "./button";
import { Input } from "./input";
import { Label } from "./label";

interface CopyableSecretProps {
  secret: string;
  label?: string;
}

export function CopyableSecret({ secret, label }: CopyableSecretProps) {
  const [copied, setCopied] = useState(false);
  const id = useId();

  const copy = async () => {
    await navigator.clipboard.writeText(secret);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="rounded-lg border border-warning/30 bg-warning/10 p-4">
      {label && <p className="text-sm font-medium text-warning">{label}</p>}
      <div className="mt-2 flex items-center gap-2">
        <div className="flex-1">
          <Label htmlFor={id} className="sr-only">
            {label || "Secret"}
          </Label>
          <Input id={id} readOnly value={secret} className="font-mono" />
        </div>
        <Button variant="secondary" size="sm" onClick={copy} className="text-warning">
          {copied ? "Copied" : "Copy"}
        </Button>
      </div>
    </div>
  );
}
