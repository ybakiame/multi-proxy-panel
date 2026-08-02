import type { ComponentType, SVGProps } from "react";
import {
  AdjustmentsHorizontalIcon,
  BeakerIcon,
  Cog6ToothIcon,
  GlobeAltIcon,
  ServerStackIcon,
  Squares2X2Icon,
  WrenchScrewdriverIcon,
} from "@heroicons/react/24/outline";
import clsx from "clsx";
import { NavLink } from "react-router-dom";

interface NavItem {
  to: string;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}

const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "仪表盘", icon: Squares2X2Icon },
  { to: "/nodes", label: "订阅", icon: ServerStackIcon },
  { to: "/mitm", label: "MITM", icon: BeakerIcon },
  { to: "/scripts", label: "脚本", icon: WrenchScrewdriverIcon },
  { to: "/override", label: "覆写", icon: AdjustmentsHorizontalIcon },
  { to: "/settings", label: "设置", icon: Cog6ToothIcon },
];

export function Sidebar() {
  return (
    <aside className="flex w-56 shrink-0 flex-col gap-4 border-r border-border/60 bg-surface p-4">
      <div className="flex items-center gap-2 px-2">
        <div className="flex size-8 items-center justify-center rounded-lg bg-primary text-white">
          <GlobeAltIcon className="size-5" />
        </div>
        <span className="text-sm font-semibold">ProxyPanel</span>
      </div>

      <nav className="flex flex-col gap-1" aria-label="主导航">
        {NAV_ITEMS.map((item) => (
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
