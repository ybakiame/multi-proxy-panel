import { useEffect, useState } from "react";
import { Alert, Button, Card, Chip } from "@heroui/react";
import { useAppStore } from "../store";

export default function Dashboard() {
  const { config, status, loading, error, loadConfig, refreshStatus, start, stop } = useAppStore();
  const [busy, setBusy] = useState<"start" | "stop" | null>(null);

  useEffect(() => {
    void loadConfig();
    void refreshStatus();
    // 状态轮询：每 2s 刷新一次运行状态。
    const timer = window.setInterval(() => {
      void refreshStatus();
    }, 2000);
    return () => window.clearInterval(timer);
  }, [loadConfig, refreshStatus]);

  const handleStart = async () => {
    setBusy("start");
    try {
      await start();
    } finally {
      setBusy(null);
    }
  };

  const handleStop = async () => {
    setBusy("stop");
    try {
      await stop();
    } finally {
      setBusy(null);
    }
  };

  const running = status?.core_running ?? false;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">仪表盘</h1>
        <p className="text-sm text-muted">代理核心运行状态与启停控制</p>
      </div>

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      <div className="grid gap-4 lg:grid-cols-3">
        <Card>
          <Card.Header>
            <Card.Title>核心状态</Card.Title>
            <Card.Description>
              {config?.core_type === "mihomo" ? "mihomo" : "sing-box"}
              {config ? ` · 混合端口 ${config.mixed_port}` : ""}
            </Card.Description>
          </Card.Header>
          <Card.Content>
            {running ? <Chip color="success">运行中</Chip> : <Chip color="danger">已停止</Chip>}
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>MITM 地址</Card.Title>
            <Card.Description>中间人代理监听地址</Card.Description>
          </Card.Header>
          <Card.Content>
            <span className="text-sm">{status?.mitm_addr ?? "未启用"}</span>
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <Card.Title>系统代理</Card.Title>
            <Card.Description>是否已接管系统代理</Card.Description>
          </Card.Header>
          <Card.Content>
            {status?.system_proxy ? <Chip color="success">已启用</Chip> : <Chip color="danger">未启用</Chip>}
          </Card.Content>
        </Card>
      </div>

      <div className="flex items-center gap-4">
        {running ? (
          <Button
            variant="danger"
            size="lg"
            isPending={loading || busy === "stop"}
            isDisabled={busy === "start"}
            onPress={() => void handleStop()}
          >
            停止代理
          </Button>
        ) : (
          <Button
            variant="primary"
            size="lg"
            isPending={loading || busy === "start"}
            isDisabled={busy === "stop"}
            onPress={() => void handleStart()}
          >
            启动代理
          </Button>
        )}
        <span className="text-sm text-muted">启动后由后端执行订阅同步并拉起核心，配置可在「设置」页修改。</span>
      </div>
    </div>
  );
}
