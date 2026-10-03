import type { ReactNode } from "react";

/**
 * 页面通用外层（ADR-0003 M5）。
 *
 * - 单列布局、内容区滚动由 App 的 `<main>` 承担，本组件给出纵向 gap 与顶部
 *   `env(safe-area-inset-top)` 适配；横向内边距含 safe-area 左右；
 * - 底部无需额外内边距：TabBar 为 flex 兄弟节点，不遮挡内容。
 */
export function PageShell({ children }: { children: ReactNode }) {
  return (
    <div
      className="flex min-h-full flex-col gap-4 pb-8 pt-[max(1.25rem,env(safe-area-inset-top))]"
      style={{
        paddingLeft: "max(1rem, env(safe-area-inset-left))",
        paddingRight: "max(1rem, env(safe-area-inset-right))",
      }}
    >
      {children}
    </div>
  );
}
