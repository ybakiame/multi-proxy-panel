import { ArrowLeftIcon } from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";

interface MobileBackHeaderProps {
  title: string;
}

/**
 * 移动端子页顶部返回栏：仅在 lg 以下显示，提供返回按钮与页面标题。
 * 桌面端由侧边栏主导航，无需此组件。
 */
export function MobileBackHeader({ title }: MobileBackHeaderProps) {
  const navigate = useNavigate();

  return (
    <div className="sticky top-0 z-40 -mx-4 mb-4 flex items-center gap-3 border-b border-border/60 bg-background/95 px-4 py-3 backdrop-blur-sm lg:hidden">
      <button
        type="button"
        onClick={() => navigate(-1)}
        className="flex size-8 items-center justify-center rounded-lg text-muted transition-colors hover:bg-surface-secondary hover:text-foreground"
        aria-label="返回"
      >
        <ArrowLeftIcon className="size-5" />
      </button>
      <span className="text-base font-semibold">{title}</span>
    </div>
  );
}
