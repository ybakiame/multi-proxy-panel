import { useTranslation } from "react-i18next";
import { MoreHorizontalIcon } from "lucide-react";
import { Button } from "./button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "./dropdown-menu";

interface RowAction {
  label: string;
  onSelect: () => void;
  destructive?: boolean;
  disabled?: boolean;
}

interface RowActionsProps {
  label: string;
  actions: RowAction[];
  primaryAction?: RowAction;
}

export function RowActions({ label, actions, primaryAction }: RowActionsProps) {
  const { t } = useTranslation();

  return (
    <div className="flex items-center justify-end gap-1">
      {primaryAction && (
        <Button
          size="sm"
          variant="ghost"
          disabled={primaryAction.disabled}
          onClick={primaryAction.onSelect}
        >
          {primaryAction.label}
        </Button>
      )}
      <DropdownMenu>
        <DropdownMenuTrigger
          render={
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label={t("common.rowActions", { name: label })}
            />
          }
        >
          <MoreHorizontalIcon aria-hidden="true" />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end" className="min-w-40">
          {actions.map((action) => (
            <DropdownMenuItem
              key={action.label}
              disabled={action.disabled}
              variant={action.destructive ? "destructive" : "default"}
              onClick={action.onSelect}
            >
              {action.label}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
