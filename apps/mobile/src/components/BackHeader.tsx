import type { ReactNode } from "react";
import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";

interface BackHeaderProps {
  /** 页面标题。 */
  title: string;
  /** 右侧动作区（如「添加」按钮），高度与返回键一致（≥48px）；缺省时用等宽占位保持标题居中。 */
  action?: ReactNode;
  /**
   * 显式返回目标路径（replace 跳转）；缺省 `navigate(-1)`。
   *
   * 内嵌 iframe 的页面（如 `/panel` 的 zashboard）必须显式指定：iframe 内的 hash 路由
   * 跳转会进入联合会话历史，`navigate(-1)` 只会回退 iframe 内部的 hash 条目、主文档
   * URL 不变（React Router 无感知），表现为「无法返回 App 界面」。
   */
  backTo?: string;
}

/**
 * 二级页返回头（ADR-0003 M5）。
 *
 * - 左返回箭头（默认 `navigate(-1)`；含 iframe 的页面经 `backTo` 显式指定目标，触达区
 *   ≥48px）+ 居中标题 + 可选右侧动作区；
 * - `sticky top-0` 相对 App 的滚动 `<main>` 吸顶；顶部 `env(safe-area-inset-top)` 适配状态栏，
 *   左右 safe-area 内边距；底部留 1px 分隔线。
 * - 订阅管理页复用（右侧「添加」动作）。
 */
export function BackHeader({ title, action, backTo }: BackHeaderProps) {
  const navigate = useNavigate();
  return (
    <header
      className="sticky top-0 z-10 border-b border-border/60 bg-background/95 pt-[env(safe-area-inset-top)] backdrop-blur-md"
      style={{
        paddingLeft: "max(0.5rem, env(safe-area-inset-left))",
        paddingRight: "max(0.5rem, env(safe-area-inset-right))",
      }}
    >
      <div className="relative flex items-center">
        <button
          type="button"
          onClick={() => (backTo ? navigate(backTo, { replace: true }) : navigate(-1))}
          aria-label="返回"
          className="relative z-10 flex h-12 w-12 shrink-0 items-center justify-center rounded-lg text-foreground active:opacity-70"
        >
          <ArrowLeftIcon className="size-6" aria-hidden="true" />
        </button>
        {/* 标题绝对居中于整个头部，避免左右两侧宽度不对称造成偏移 */}
        <h1 className="pointer-events-none absolute inset-x-0 top-1/2 -translate-y-1/2 truncate px-16 text-center text-base font-semibold text-foreground">
          {title}
        </h1>
        {action ? (
          <div className="relative z-10 ml-auto flex h-12 shrink-0 items-center gap-1 pl-2">{action}</div>
        ) : (
          <div className="w-12 shrink-0" aria-hidden="true" />
        )}
      </div>
    </header>
  );
}
