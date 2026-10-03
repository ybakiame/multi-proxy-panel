import type { ReactNode } from "react";
import { BackHeader } from "./BackHeader";

interface SubPageShellProps {
  /** BackHeader 标题。 */
  title: string;
  /** 显式返回目标（透传 BackHeader；含 iframe 的页面必须显式指定）。 */
  backTo?: string;
  /** BackHeader 右侧动作区（如保存 / 添加按钮）。 */
  action?: ReactNode;
  children: ReactNode;
}

/**
 * 二级页统一骨架（BackHeader + 内容列）。
 *
 * 收拢各二级页此前各自手写的外层包装：底部 safe-area 内边距、内容区横向
 * safe-area、纵向 `gap-4 pt-3` 间距。二级页只写自身内容；三主 Tab 页仍用
 * PageShell（自带标题区、无 BackHeader）。
 */
export function SubPageShell({ title, backTo, action, children }: SubPageShellProps) {
  return (
    <div className="flex min-h-full flex-col pb-[max(1.5rem,env(safe-area-inset-bottom))]">
      <BackHeader title={title} backTo={backTo} action={action} />
      <div
        className="flex min-h-full flex-1 flex-col gap-4 pt-3"
        style={{
          paddingLeft: "max(1rem, env(safe-area-inset-left))",
          paddingRight: "max(1rem, env(safe-area-inset-right))",
        }}
      >
        {children}
      </div>
    </div>
  );
}
