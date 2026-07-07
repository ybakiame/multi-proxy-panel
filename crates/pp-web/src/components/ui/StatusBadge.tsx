import { Chip } from "@heroui/react";

interface StatusBadgeProps {
  status: string;
}

export function StatusBadge({ status }: StatusBadgeProps) {
  const s = status.toLowerCase();
  let color:
    | "default"
    | "primary"
    | "secondary"
    | "success"
    | "warning"
    | "danger"
    | undefined = "default";

  if (["online", "active", "enabled"].includes(s)) color = "success";
  else if (["offline", "inactive", "disabled", "expired"].includes(s)) color = "danger";
  else if (["connecting", "on_hold", "limited", "degraded"].includes(s)) color = "warning";
  else if (["maintenance"].includes(s)) color = "secondary";

  return (
    <Chip color={color} size="sm" variant="flat">
      {status}
    </Chip>
  );
}
