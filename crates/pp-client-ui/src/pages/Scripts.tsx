import { Alert, Card } from "@heroui/react";

export default function Scripts() {
  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">脚本</h1>
        <p className="text-sm text-muted">MITM 抓包脚本的管理与调度（Phase 2）</p>
      </div>

      <Card className="max-w-xl">
        <Card.Header>
          <Card.Title>脚本管理</Card.Title>
          <Card.Description>按方言生成 Surge / QuantumultX / Loon 抓包脚本</Card.Description>
        </Card.Header>
        <Card.Content>
          <Alert status="accent">
            <Alert.Indicator />
            <Alert.Content>
              <Alert.Title>占位页</Alert.Title>
              <Alert.Description>
                Phase 2 将基于配置中的 `mitm_script_dialect`（Surge / QuantumultX / Loon）
                生成并调度抓包脚本，本页暂不提供具体功能。
              </Alert.Description>
            </Alert.Content>
          </Alert>
        </Card.Content>
      </Card>
    </div>
  );
}
