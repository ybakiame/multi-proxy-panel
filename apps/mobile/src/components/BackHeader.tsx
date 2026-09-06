import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";

interface BackHeaderProps {
  /** 页面标题。 */
  title: string;
}

/**
 * 二级页返回头（ADR-0003 M5）。
 *
 * - 左返回箭头（`navigate(-1)`，触达区 ≥48px）+ 居中标题；
 * - `sticky top-0` 相对 App 的滚动 `<main>` 吸顶；顶部 `env(safe-area-inset-top)` 适配状态栏，
 *   左右 safe-area 内边距；底部留 1px 分隔线。
 * - 本页与后续代理二级页复用。
 */
export function BackHeader({ title }: BackHeaderProps) {
  const navigate = useNavigate();
  return (
    <header
      className="sticky top-0 z-10 flex items-center border-b border-border/60 bg-background/95 pt-[env(safe-area-inset-top)] backdrop-blur-md"
      style={{
        paddingLeft: "max(0.5rem, env(safe-area-inset-left))",
        paddingRight: "max(0.5rem, env(safe-area-inset-right))",
      }}
    >
      <button
        type="button"
        onClick={() => navigate(-1)}
        aria-label="返回"
        className="-ml-1 flex min-h-12 min-w-12 shrink-0 items-center justify-center rounded-lg text-foreground active:opacity-70"
      >
        <ArrowLeftIcon className="size-6" aria-hidden="true" />
      </button>
      <h1 className="min-w-0 flex-1 truncate pr-8 text-center text-base font-semibold text-foreground">{title}</h1>
    </header>
  );
}
