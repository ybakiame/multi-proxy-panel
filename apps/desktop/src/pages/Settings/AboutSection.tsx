import { Button, Card } from "@heroui/react";
import { useNavigate } from "react-router-dom";
import { APP_VERSION } from "./useSettingsConfig";

export default function AboutSection() {
  const navigate = useNavigate();

  return (
    <>
      <Card>
        <Card.Header>
          <Card.Title>关于应用</Card.Title>
          <Card.Description>ProxyPanel 客户端信息</Card.Description>
        </Card.Header>
        <Card.Content className="flex flex-col gap-4">
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted">版本号</span>
            <span className="text-sm font-medium">{APP_VERSION}</span>
          </div>
          <div className="flex items-center justify-between">
            <span className="text-sm text-muted">项目链接</span>
            <Button
              variant="secondary"
              size="sm"
              onPress={() => {
                window.open("https://github.com/ybakiame/multi-proxy-panel", "_blank");
              }}
            >
              GitHub
            </Button>
          </div>
        </Card.Content>
      </Card>

      {/* 日志入口 */}
      <Card className="cursor-pointer transition-colors hover:bg-surface-secondary" onClick={() => navigate("/logs")}>
        <Card.Content className="flex items-center justify-between p-4">
          <div className="flex flex-col gap-0.5">
            <span className="text-sm font-medium">日志</span>
            <span className="text-xs text-muted">查看后端与前端错误日志</span>
          </div>
          <span className="text-sm text-muted">→</span>
        </Card.Content>
      </Card>
    </>
  );
}
