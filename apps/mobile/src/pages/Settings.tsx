import { Card } from "@heroui/react";
import { PageShell } from "../components/PageShell";

/**
 * 设置（占位页，ADR-0003 M5）：完整设置项（Clash API / VPN 通知 / 订阅管理等）后续批次实现。
 */
export default function Settings() {
  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">设置</h1>
        <p className="text-sm text-muted">客户端全局设置</p>
      </div>
      <Card>
        <Card.Content className="flex flex-col items-center justify-center gap-2 py-12 text-center">
          <span className="text-sm text-muted">更多设置即将上线</span>
          <span className="text-xs text-muted/80">Clash API、VPN 通知等设置项将在后续版本提供</span>
        </Card.Content>
      </Card>
    </PageShell>
  );
}
