import { Button } from "@heroui/react";
import { cn } from "../../utils/cn";

interface PageHeaderProps {
  title: string;
  action?: {
    label: string;
    onClick: () => void;
  };
  className?: string;
}

export function PageHeader({ title, action, className }: PageHeaderProps) {
  return (
    <div className={cn("mb-6 flex items-center justify-between", className)}>
      <h1 className="text-2xl font-bold">{title}</h1>
      {action && <Button onPress={action.onClick}>{action.label}</Button>}
    </div>
  );
}
