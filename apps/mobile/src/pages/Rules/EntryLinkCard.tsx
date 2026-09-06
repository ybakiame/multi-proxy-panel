import type { ReactNode } from "react";
import { Card } from "@heroui/react";
import { ChevronRightIcon } from "@heroicons/react/24/outline";

interface EntryLinkCardProps {
  /** 卡片图标（HeroUI 图标节点，父层自选语义图标）。 */
  icon: ReactNode;
  title: string;
  description: string;
  onPress: () => void;
}

/**
 * 规则主页入口卡（ADR-0003 M5.4 拆分后的两个二级页入口）。
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
        <span className="flex size-12 shrink-0 items-center justify-center rounded-xl bg-accent/10 text-accent">
          {icon}
        </span>
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="truncate text-sm font-semibold text-foreground">{title}</span>
          <span className="truncate text-xs text-muted">{description}</span>
        </span>
        <ChevronRightIcon className="size-5 shrink-0 text-muted" aria-hidden="true" />
      </button>
    </Card>
  );
}
