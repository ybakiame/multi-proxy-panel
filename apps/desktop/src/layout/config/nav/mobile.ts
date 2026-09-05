import type { ComponentType, SVGProps } from "react";
import { Cog6ToothIcon, GlobeAltIcon, Squares2X2Icon, WrenchScrewdriverIcon } from "@heroicons/react/24/outline";

/** 移动端底部 Tab 项：目标路由、展示名与图标。 */
export interface MobileTabItem {
  to: string;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  /** 该 Tab 是否要求 `capabilities.scripts_remote` 为 true；Android 无远程脚本时隐藏「工具」入口。 */
  requiresScriptsRemote?: boolean;
  /** 该 Tab 是否要求 `capabilities.mitm` 为 true；影响工具页内卡片显隐，Tab 本身始终显示。 */
  requiresMitm?: boolean;
}

/** 移动端 4 个主 Tab（底部 TabBar）。 */
export const MOBILE_TAB_ITEMS: MobileTabItem[] = [
  { to: "/", label: "首页", icon: Squares2X2Icon },
  { to: "/proxies", label: "策略组", icon: GlobeAltIcon },
  { to: "/tools", label: "工具", icon: WrenchScrewdriverIcon },
  { to: "/settings", label: "设置", icon: Cog6ToothIcon },
];
