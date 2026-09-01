import clsx from "clsx";
import { NavLink } from "react-router-dom";
import { useCapabilities } from "../hooks/useCapabilities";
import { MOBILE_TAB_ITEMS } from "./nav/mobile";

/**
 * 移动端底部 TabBar：`lg`（1024px）以下显示，桌面端隐藏。
 * HeroUI 风格，图标+文字，激活态高亮，safe-area 适配。
 */
export function MobileTabBar() {
  const { data: capabilities } = useCapabilities();

  const items = MOBILE_TAB_ITEMS.filter((item) => {
    if (!capabilities) return true;
    const caps = capabilities.capabilities;
    // Tab 本身始终显示；工具页内部卡片按能力显隐
    if (item.requiresScriptsRemote && !caps.scripts_remote && !caps.mitm) {
      // 若 Android 既无脚本也无 MITM，工具页无内容，但仍保留 Tab 作为占位
      return true;
    }
    return true;
  });

  return (
    <nav
      className="fixed bottom-0 left-0 right-0 z-50 border-t border-border/60 bg-surface lg:hidden"
      style={{ paddingBottom: "env(safe-area-inset-bottom)" }}
      aria-label="底部导航"
    >
      <div className="flex items-center justify-around">
        {items.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              clsx(
                "flex flex-1 flex-col items-center gap-0.5 py-2 text-xs transition-colors",
                isActive ? "text-primary" : "text-muted",
              )
            }
          >
            {({ isActive }) => (
              <>
                <div
                  className={clsx(
                    "flex items-center justify-center rounded-lg p-1 transition-colors",
                    isActive ? "bg-primary/10" : "bg-transparent",
                  )}
                >
                  <item.icon className="size-5" aria-hidden="true" />
                </div>
                <span className="font-medium">{item.label}</span>
              </>
            )}
          </NavLink>
        ))}
      </div>
    </nav>
  );
}
