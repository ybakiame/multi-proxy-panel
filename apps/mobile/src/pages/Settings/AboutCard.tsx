import { Card } from "@heroui/react";

/**
 * 关于（ADR-0003 M5.3）：移动端无版本号数据源（desktop 以构建期注入读取），
 * 故仅展示应用名 + 一行说明，不显示版本。
 */
export function AboutCard() {
  return (
    <Card>
      <Card.Header>
        <Card.Title>ProxyPanel Mobile</Card.Title>
        <Card.Description>关于应用</Card.Description>
      </Card.Header>
      <Card.Content>
        <p className="text-sm leading-relaxed text-muted">
          基于 sing-box 内核的 Android 代理客户端：同步 ProxyPanel Hub 订阅并本地合成配置， 经系统 VPN 转发流量。
        </p>
      </Card.Content>
    </Card>
  );
}
