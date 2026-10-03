import type { ReactNode } from "react";
import { Navbar, NavbarBackLink } from "konsta/react";
import { useNavigate } from "react-router-dom";

interface BackHeaderProps {
  /** 页面标题。 */
  title: string;
  /** 右侧动作区（如「添加」按钮）。 */
  action?: ReactNode;
  /**
   * 显式返回目标路径（replace 跳转）；缺省 `navigate(-1)`。
   *
   * 内嵌 iframe 的页面（如 `/panel` 的 zashboard）必须显式指定：iframe 内的 hash 路由
   * 跳转会进入联合会话历史，`navigate(-1)` 只会回退 iframe 内部的 hash 条目、主文档
   * URL 不变（React Router 无感知），表现为「无法返回 App 界面」。
   */
  backTo?: string;
}

/**
 * 二级页返回头（Konsta UI `Navbar`，随 iOS/Material 主题切换观感）。
 *
 * - 左侧返回（`NavbarBackLink` 图标形态；含 iframe 的页面经 `backTo` 显式指定目标）；
 * - 标题居中（iOS 惯例；Material 主题由 Konsta 自动左对齐）；
 * - `sticky top-0` 相对 App 的滚动 `<main>` 吸顶，顶部状态栏 safe-area 由 Konsta
 *   Navbar 内置 `--k-safe-area-top` 处理；iOS 自带毛玻璃背景与底部分隔线。
 */
export function BackHeader({ title, action, backTo }: BackHeaderProps) {
  const navigate = useNavigate();
  return (
    <Navbar
      title={title}
      centerTitle
      left={
        <NavbarBackLink
          text="返回"
          showText={false}
          aria-label="返回"
          onClick={() => (backTo ? navigate(backTo, { replace: true }) : navigate(-1))}
        />
      }
      right={action}
    />
  );
}
