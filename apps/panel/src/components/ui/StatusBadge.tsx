import { useTranslation } from "react-i18next";
import { Badge } from "./badge";

interface StatusBadgeProps {
  status: string;
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const { t } = useTranslation();
  const s = status.toLowerCase();
  let variant: "secondary" | "destructive" = "secondary";
  let className = "";

  if (["online", "active", "enabled"].includes(s)) className = "bg-success/10 text-success";
  else if (["offline", "inactive", "disabled", "expired"].includes(s)) variant = "destructive";
  else if (["connecting", "on_hold", "limited", "degraded"].includes(s)) {
    className = "bg-warning/10 text-warning";
  }

  const labelKey = `status.${s}`;
  const label = t(labelKey, { defaultValue: status });

  return (
    <Badge variant={variant} className={className}>
      {label}
    </Badge>
  );
}
