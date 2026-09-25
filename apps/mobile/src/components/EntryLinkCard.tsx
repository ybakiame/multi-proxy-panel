import type { ReactNode } from "react";
import { Card } from "./ui";
import { ChevronRightIcon } from "@heroicons/react/24/outline";

interface EntryLinkCardProps {
  /** 卡片图标（HeroUI 图标节点，父层自选语义图标）。 */
  icon: ReactNode;
  title: string;
  description: string;
  onPress: () => void;
}

/**
 * 二级页入口卡（自 Config 目录提取共用）。
 *
 * 大触达列表项：图标（浅色圆角底）+ 标题 + 描述 + chevron，整卡可点跳转子页。
 */
export function EntryLinkCard({ icon, title, description, onPress }: EntryLinkCardProps) {
  return (
    <Card>
      <button
        type="button"
        onClick={onPress}
        aria-label={title}
        className="flex w-full items-center gap-3 rounded-xl py-1 pl-1 pr-2 text-left active:opacity-80"
      >
        <span className="flex size-12 shrink-0 items-center justify-center rounded-xl bg-primary/10 text-primary">
          {icon}
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-semibold text-zinc-900 dark:text-zinc-100">{title}</span>
          <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">{description}</span>
        </span>
        <ChevronRightIcon className="size-5 shrink-0 text-zinc-400" aria-hidden="true" />
      </button>
    </Card>
  );
}
