import type { ReactNode } from "react";
import { Navbar, Button } from "@pp/ui";
import { ChevronLeftIcon } from "@heroicons/react/24/outline";
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

/** 二级页返回栏，@pp/ui Navbar 负责安全区与 iOS/Material 呈现。 */
export function BackHeader({ title, action, backTo }: BackHeaderProps) {
  const navigate = useNavigate();
  return (
    <Navbar
      title={title}
      left={
        <Button
          variant="ghost"
          isIconOnly
          aria-label="返回"
          onPress={() => (backTo ? navigate(backTo, { replace: true }) : navigate(-1))}
        >
          <ChevronLeftIcon className="size-6" />
        </Button>
      }
      right={action}
    />
  );
}
