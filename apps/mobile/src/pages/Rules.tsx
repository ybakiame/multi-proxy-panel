import { Card } from "@heroui/react";
import { PageShell } from "../components/PageShell";

/**
 * 规则管理（占位页，ADR-0003 M5）：完整功能（默认规则模板 / 自定义规则）后续批次实现。
 */
export default function Rules() {
  return (
    <PageShell>
      <div>
        <h1 className="text-xl font-semibold">规则管理</h1>
        <p className="text-sm text-muted">管理流量路由规则与规则集</p>
      </div>
      <Card>
        <Card.Content className="flex flex-col items-center justify-center gap-2 py-12 text-center">
          <span className="text-sm text-muted">默认规则模板与自定义规则即将上线</span>
          <span className="text-xs text-muted/80">后续版本支持本地规则覆盖、规则集订阅与模板应用</span>
        </Card.Content>
      </Card>
    </PageShell>
  );
}
