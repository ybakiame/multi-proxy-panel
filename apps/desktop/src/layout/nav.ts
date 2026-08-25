import type { ComponentType, SVGProps } from "react";
import {
  AdjustmentsHorizontalIcon,
  BeakerIcon,
  Cog6ToothIcon,
  DocumentTextIcon,
  ServerStackIcon,
  Squares2X2Icon,
  WrenchScrewdriverIcon,
} from "@heroicons/react/24/outline";

/** 单个导航项：目标路由、展示名与图标。 */
export interface NavItem {
  to: string;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
}

/** 主导航项：桌面侧栏与移动端抽屉共用（to/label/icon）。 */
export const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "仪表盘", icon: Squares2X2Icon },
  { to: "/nodes", label: "订阅", icon: ServerStackIcon },
  { to: "/mitm", label: "MITM", icon: BeakerIcon },
  { to: "/scripts", label: "脚本", icon: WrenchScrewdriverIcon },
  { to: "/override", label: "覆写", icon: AdjustmentsHorizontalIcon },
  { to: "/logs", label: "日志", icon: DocumentTextIcon },
  { to: "/settings", label: "设置", icon: Cog6ToothIcon },
];
