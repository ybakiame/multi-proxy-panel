import { Button, Card } from "@heroui/react";
import { APP_VERSION } from "./useSettingsConfig";

/**
 * 关于应用卡（版本号 / 项目链接）。
 *
 * 日志入口已移除（2026-10）：侧边栏主导航本就有「日志」页，设置页内的跳转卡片
 * 只是重复入口，无独立意义。
 */
export default function AboutSection() {
  return (
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
  );
}
