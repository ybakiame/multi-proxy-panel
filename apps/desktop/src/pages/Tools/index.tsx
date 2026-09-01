import { Card } from "@heroui/react";
import {
  AdjustmentsHorizontalIcon,
  ArrowPathIcon,
  DocumentTextIcon,
  ShieldCheckIcon,
  WrenchScrewdriverIcon,
} from "@heroicons/react/24/outline";
import { useNavigate } from "react-router-dom";
import { useCapabilities } from "../../hooks/useCapabilities";

interface ToolCardProps {
  title: string;
  description: string;
  icon: React.ComponentType<React.SVGProps<SVGSVGElement>>;
  onPress: () => void;
}

function ToolCard({ title, description, icon: Icon, onPress }: ToolCardProps) {
  return (
    <Card className="cursor-pointer transition-colors hover:bg-surface-secondary" onClick={() => onPress()}>
      <Card.Content className="flex items-center gap-4 p-4">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary">
          <Icon className="size-5" aria-hidden="true" />
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="text-sm font-medium">{title}</span>
          <span className="text-xs text-muted">{description}</span>
        </div>
      </Card.Content>
    </Card>
  );
}

/**
 * 工具 Hub 页：规则、脚本、覆写、日志入口卡片。
 * 按平台能力显隐（如 Android 无 MITM/脚本远程 Tab 时隐藏对应卡片）。
 */
export default function Tools() {
  const navigate = useNavigate();
  const { data: capabilities } = useCapabilities();
  const caps = capabilities?.capabilities;

  const showRules = true;
  const showScripts = caps?.scripts_remote ?? true;
  const showOverride = true;
  const showLogs = true;
  const showConnections = true;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">工具</h1>
        <p className="text-sm text-muted">规则、脚本、覆写、日志与连接管理</p>
      </div>

      <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
        {showConnections && (
          <ToolCard
            title="连接"
            description="查看当前活跃连接与已关闭连接记录"
            icon={ArrowPathIcon}
            onPress={() => navigate("/connections")}
          />
        )}
        {showRules && (
          <ToolCard
            title="规则"
            description="本地规则卡片、场景模板与规则集订阅管理"
            icon={ShieldCheckIcon}
            onPress={() => navigate("/rules")}
          />
        )}
        {showScripts && (
          <ToolCard
            title="脚本"
            description="远程脚本 / 配置片段订阅、定时任务调度与三方配置导入"
            icon={WrenchScrewdriverIcon}
            onPress={() => navigate("/scripts")}
          />
        )}
        {showOverride && (
          <ToolCard
            title="覆写"
            description="按核心类型维护覆写模板，在订阅页关联后随订阅生效"
            icon={AdjustmentsHorizontalIcon}
            onPress={() => navigate("/override")}
          />
        )}
        {showLogs && (
          <ToolCard
            title="日志"
            description="后端与前端错误日志（内存环形缓冲，最新在前）"
            icon={DocumentTextIcon}
            onPress={() => navigate("/logs")}
          />
        )}
      </div>
    </div>
  );
}
