import { AdjustmentsHorizontalIcon, Cog6ToothIcon, Squares2X2Icon } from "@heroicons/react/24/outline";
import { NavLink } from "react-router-dom";

/** 底部导航 Tab（`end` 仅用于根路径：`/` 精确匹配，避免「/config」「/settings」前缀命中）。 */
export const TABS = [
  { to: "/", label: "首页", icon: Squares2X2Icon, end: true },
  { to: "/config", label: "配置", icon: AdjustmentsHorizontalIcon, end: false },
  { to: "/settings", label: "设置", icon: Cog6ToothIcon, end: false },
] as const;

/**
 * 移动端底部固定 TabBar（ADR-0003 M5）。
 *
 * - 3 个 Tab：首页（仪表盘）/ 配置管理 / 设置（后两者本任务为占位页）；
 * - `NavLink` 激活态高亮（text-primary）；
 * - 触达区整块 ≥56px（min-h-14），底部 `env(safe-area-inset-bottom)` 适配系统手势条。
 */
export function TabBar() {
  return (
    <nav
      className="flex shrink-0 border-t border-border/60 bg-surface pb-[env(safe-area-inset-bottom)]"
      aria-label="底部导航"
    >
      {TABS.map(({ to, label, icon: Icon, end }) => (
        <NavLink
          key={to}
          to={to}
          end={end}
          className={({ isActive }) =>
            `flex min-h-14 flex-1 flex-col items-center justify-center gap-0.5 px-2 text-xs transition-colors ${
              isActive ? "text-primary" : "text-muted hover:text-foreground"
            }`
          }
        >
          <Icon className="size-6" aria-hidden="true" />
          <span className="leading-none">{label}</span>
        </NavLink>
      ))}
    </nav>
  );
}
