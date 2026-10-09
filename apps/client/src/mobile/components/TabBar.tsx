import { AdjustmentsHorizontalIcon, Cog6ToothIcon, Squares2X2Icon } from "@heroicons/react/24/outline";
import { Tabbar, TabbarLink, ToolbarPane } from "konsta/react";
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
 * 移动端底部 TabBar（Konsta UI `Tabbar`，官方示例同款结构：fixed 定位 + `ToolbarPane`）。
 *
 * - iOS 主题：`ToolbarPane` 呈现悬浮玻璃药丸（rounded-full + 毛玻璃），激活项
 *   附带滑动高亮；Material 主题：`ToolbarPane` 为 `contents`，退化为贴底 MD 底栏；
 * - fixed 悬浮于内容之上（内容滚动可穿透毛玻璃），滚动区底部留白由 AppContent
 *   的 `pb-safe-24` 承担；safe-area 由 Konsta `--k-safe-area-bottom` 内置处理
 *   （依赖根部 `.safe-areas` 类）；
 * - 激活判定：`/` 精确匹配，其余前缀匹配（二级页不渲染本组件，见 App.tsx）。
 */
export function TabBar() {
  const location = useLocation();
  const navigate = useNavigate();
  return (
    <Tabbar labels icons className="fixed bottom-0 left-0 right-0 z-30" aria-label="底部导航">
      <ToolbarPane>
        {TABS.map(({ to, label, icon: Icon, end }) => (
          <TabbarLink
            key={to}
            active={isTabActive(location.pathname, to, end)}
            icon={<Icon className="size-6" aria-hidden="true" />}
            label={label}
            onClick={() => navigate(to)}
            aria-label={label}
          />
        ))}
      </ToolbarPane>
    </Tabbar>
  );
}
