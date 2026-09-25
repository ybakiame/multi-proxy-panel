import { AdjustmentsHorizontalIcon, Cog6ToothIcon, Squares2X2Icon } from "@heroicons/react/24/outline";
import { Tabbar, TabbarLink } from "konsta/react";
import { useLocation, useNavigate } from "react-router-dom";

/** 底部导航 Tab（`end` 仅用于根路径：`/` 精确匹配，避免「/config」「/settings」前缀命中）。 */
export const TABS = [
  { to: "/", label: "首页", icon: Squares2X2Icon, end: true },
  { to: "/config", label: "配置", icon: AdjustmentsHorizontalIcon, end: false },
  { to: "/settings", label: "设置", icon: Cog6ToothIcon, end: false },
] as const;

/** 路径是否属于该 Tab（end 精确匹配；否则前缀匹配）。 */
function isTabActive(pathname: string, to: string, end: boolean): boolean {
  if (end) return pathname === to;
  return pathname === to || pathname.startsWith(`${to}/`);
}

/**
 * 移动端底部固定 TabBar（Konsta UI `Tabbar`，随 iOS/Material 主题切换观感）。
 *
 * - 3 个 Tab：首页（仪表盘）/ 配置管理 / 设置；
 * - 激活态由 `TabbarLink active` 呈现（主题主色）；点击经 `useNavigate` 路由跳转；
 * - 底部 safe-area 内边距由 Konsta Toolbar/Tabbar 内置 `--k-safe-area-bottom` 处理。
 */
export function TabBar() {
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <Tabbar labels icons className="shrink-0" aria-label="底部导航">
      {TABS.map(({ to, label, icon: Icon, end }) => (
        <TabbarLink
          key={to}
          active={isTabActive(location.pathname, to, end)}
          icon={<Icon className="size-6" aria-hidden="true" />}
          label={label}
          linkProps={{ onClick: () => navigate(to), "aria-label": label }}
        />
      ))}
    </Tabbar>
  );
}
