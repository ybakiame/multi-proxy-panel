import type { ComponentType, SVGProps } from "react";
import {
  AdjustmentsHorizontalIcon,
  BeakerIcon,
  Cog6ToothIcon,
  DocumentTextIcon,
  ServerStackIcon,
  ShieldCheckIcon,
  Squares2X2Icon,
  WrenchScrewdriverIcon,
} from "@heroicons/react/24/outline";

/** 单个导航项：目标路由、展示名与图标。 */
export interface NavItem {
  to: string;
  label: string;
  icon: ComponentType<SVGProps<SVGSVGElement>>;
  /** 该导航项是否要求 `capabilities.scripts_remote` 为 true；Android 下隐藏。 */
  requiresScriptsRemote?: boolean;
  /** 该导航项是否要求 `capabilities.mitm` 为 true；Android 下隐藏。 */
  requiresMitm?: boolean;
  /** 该导航项是否要求 `capabilities.cron_tasks` 为 true；阶段③后始终 true。 */
  requiresCronTasks?: boolean;
}

/** 主导航项：桌面侧栏与移动端抽屉共用（to/label/icon）。
 *
 * 能力门控字段说明：
 * - `requiresMitm=true`：MITM 页，Android 下 `capabilities.mitm=false` 时隐藏
 * - `requiresScriptsRemote=true`：脚本页中依赖远程资源的功能，Android 下隐藏
 * - `requiresCronTasks=true`：脚本页（定时任务），阶段③后始终显示
 *
 * 阶段③解耦后：脚本页在 Android 显示，但仅展示「定时任务」Tab。
 */
export const NAV_ITEMS: NavItem[] = [
  { to: "/", label: "仪表盘", icon: Squares2X2Icon },
  { to: "/nodes", label: "订阅", icon: ServerStackIcon },
  { to: "/rules", label: "规则", icon: ShieldCheckIcon },
  { to: "/mitm", label: "MITM", icon: BeakerIcon, requiresMitm: true },
  { to: "/scripts", label: "脚本", icon: WrenchScrewdriverIcon, requiresCronTasks: true },
  { to: "/override", label: "覆写", icon: AdjustmentsHorizontalIcon },
  { to: "/logs", label: "日志", icon: DocumentTextIcon },
  { to: "/settings", label: "设置", icon: Cog6ToothIcon },
];
