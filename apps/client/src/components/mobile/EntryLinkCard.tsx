import type { ReactNode } from "react";
import { ChevronRightIcon } from "@heroicons/react/24/outline";

export interface EntryLinkItem {
  icon: ReactNode;
  title: string;
  description: string;
  onPress: () => void;
}
export function EntryLinkList({ entries }: { entries: EntryLinkItem[] }) {
  return (
    <ul className="overflow-hidden rounded-2xl bg-surface">
      {entries.map(({ icon, title, description, onPress }) => (
        <li key={title} className="border-b border-separator last:border-0">
          <button
            type="button"
            onClick={onPress}
            className="flex min-h-16 w-full items-center gap-3 px-4 py-3 text-left outline-none active:bg-default focus-visible:bg-default"
          >
            <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
              {icon}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block text-sm font-medium text-foreground">{title}</span>
              <span className="block text-xs text-muted">{description}</span>
            </span>
            <ChevronRightIcon aria-hidden className="size-4 shrink-0 text-muted" />
          </button>
        </li>
      ))}
    </ul>
  );
}
