import { GlobeAltIcon } from "@heroicons/react/24/outline";
import clsx from "clsx";
import { NavLink } from "react-router-dom";
import { useCapabilities } from "../hooks/useCapabilities";
import { NAV_ITEMS } from "./nav";

function visibleNavItems(capabilities: ReturnType<typeof useCapabilities>["data"]) {
  if (!capabilities) return NAV_ITEMS;
  const caps = capabilities.capabilities;
  return NAV_ITEMS.filter((item) => {
    if (item.requiresMitm && !caps.mitm) return false;
    if (item.requiresScriptsRemote && !caps.scripts_remote) return false;
    if (item.requiresCronTasks && !caps.cron_tasks) return false;
    return true;
  });
}

/**
 * 桌面端侧栏：`lg`（1024px）及以上常显，`lg` 以下隐藏
 * （导航收进移动端顶栏的抽屉，见 `./MobileNav`）。
 */
export function Sidebar() {
  const { data: capabilities } = useCapabilities();
  const items = visibleNavItems(capabilities);

  return (
    <aside className="hidden w-56 shrink-0 flex-col gap-4 border-r border-border/60 bg-surface p-4 lg:flex">
      <div className="flex items-center gap-2 px-2">
        <div className="flex size-8 items-center justify-center rounded-lg bg-primary text-white">
          <GlobeAltIcon className="size-5" />
        </div>
        <span className="text-sm font-semibold">ProxyPanel</span>
      </div>

      <nav className="flex flex-col gap-1" aria-label="主导航">
        {items.map((item) => (
          <NavLink
            key={item.to}
            to={item.to}
            end={item.to === "/"}
            className={({ isActive }) =>
              clsx(
                "flex items-center gap-2 rounded-lg px-3 py-2 text-sm transition-colors",
                isActive ? "bg-primary/10 text-primary" : "text-muted hover:bg-surface-secondary hover:text-foreground",
              )
            }
          >
            <item.icon className="size-4" aria-hidden="true" />
            {item.label}
          </NavLink>
        ))}
      </nav>
    </aside>
  );
}
