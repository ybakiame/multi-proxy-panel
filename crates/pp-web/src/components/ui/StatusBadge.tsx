import { Badge } from "@heroui/react";
import { useTranslation } from "react-i18next";

interface StatusBadgeProps {
  status: string;
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const { t } = useTranslation();
  const s = status.toLowerCase();
  let color: "default" | "accent" | "success" | "warning" | "danger" = "default";
  let variant: "primary" | "secondary" | "soft" = "soft";

  if (["online", "active", "enabled"].includes(s)) color = "success";
  else if (["offline", "inactive", "disabled", "expired"].includes(s)) color = "danger";
  else if (["connecting", "on_hold", "limited", "degraded"].includes(s)) color = "warning";
  else if (["maintenance"].includes(s)) color = "default";

  const labelKey = `status.${s}`;
  const label = t(labelKey, { defaultValue: status });

  return (
    <Badge color={color} size="sm" variant={variant}>
      {label}
    </Badge>
  );
}
