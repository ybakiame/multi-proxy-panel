import type { ReactNode } from "react";
import { List, ListItem } from "konsta/react";

/** 二级页入口项定义。 */
export interface EntryLinkItem {
  /** 卡片图标（Heroicons 节点，父层自选语义图标）。 */
  icon: ReactNode;
  title: string;
  description: string;
  onPress: () => void;
}

interface EntryLinkListProps {
  entries: EntryLinkItem[];
}

/**
 * 二级页入口列表（Konsta UI `List strong inset` 分组列表，iOS 设置页风格；
 * Material 主题下由 Konsta 自动呈现对应观感）。
 *
 * 整行可点跳转子页（`ListItem link`，iOS 自带 chevron）；图标为主色圆角方块。
 */
export function EntryLinkList({ entries }: EntryLinkListProps) {
  // Konsta List inset 自带 mx-safe-4 外边距，twMerge 无法识别 safe 缩放，
  // 普通 m-0 覆盖不掉（会与容器 padding 叠加成双倍边距），必须用 important（同 ui.tsx Card）。
  return (
    <List strong inset className="m-0!">
      {entries.map(({ icon, title, description, onPress }) => (
        <ListItem
          key={title}
          link
          title={title}
          text={description}
          onClick={onPress}
          media={
            <span className="flex size-8 items-center justify-center rounded-lg bg-primary/10 text-primary">
              {icon}
            </span>
          }
        />
      ))}
    </List>
  );
}
