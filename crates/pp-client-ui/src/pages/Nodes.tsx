import { useEffect, useState } from "react";
import { Alert, Button, Card } from "@heroui/react";
import { useAppStore } from "../store";

function maskToken(token: string): string {
  if (!token) {
    return "（未配置）";
  }
  if (token.length <= 8) {
    return "••••••••";
  }
  return `${token.slice(0, 4)}••••${token.slice(-4)}`;
}

function maskHubUrl(url: string): string {
  if (!url) {
    return "（未配置）";
  }
  try {
    const parsed = new URL(url);
    if (parsed.username) {
      parsed.username = "****";
      if (parsed.password) {
        parsed.password = "****";
      }
      return parsed.toString();
    }
    return url;
  } catch {
    return url;
  }
}

export default function Nodes() {
  const { config, loadConfig } = useAppStore();
  const [synced, setSynced] = useState(false);

  useEffect(() => {
    void loadConfig();
  }, [loadConfig]);

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">节点</h1>
        <p className="text-sm text-muted">订阅源信息与同步入口</p>
      </div>

      <Card className="max-w-xl">
        <Card.Header>
          <Card.Title>订阅源</Card.Title>
          <Card.Description>来自客户端配置，Token 已掩码显示</Card.Description>
        </Card.Header>
        <Card.Content>
          <dl className="flex flex-col gap-3 text-sm">
            <div className="flex items-center justify-between gap-4">
              <dt className="text-muted">Hub 地址</dt>
              <dd className="truncate font-mono">{maskHubUrl(config?.hub_url ?? "")}</dd>
            </div>
            <div className="flex items-center justify-between gap-4">
              <dt className="text-muted">订阅 Token</dt>
              <dd className="truncate font-mono">{maskToken(config?.sub_token ?? "")}</dd>
            </div>
            <div className="flex items-center justify-between gap-4">
              <dt className="text-muted">核心类型</dt>
              <dd>{config?.core_type === "mihomo" ? "mihomo" : "sing-box"}</dd>
            </div>
            <div className="flex items-center justify-between gap-4">
              <dt className="text-muted">混合端口</dt>
              <dd>{config?.mixed_port ?? "-"}</dd>
            </div>
          </dl>
        </Card.Content>
        <Card.Footer>
          <Button isDisabled={!config?.hub_url || !config?.sub_token} onPress={() => setSynced(true)}>
            同步订阅
          </Button>
        </Card.Footer>
      </Card>

      {synced && (
        <Alert status="accent" className="max-w-xl">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>订阅同步说明</Alert.Title>
            <Alert.Description>
              MVP 从简：订阅拉取由后端在「启动代理」时自动完成，本页暂不单独触发。请前往仪表盘启动代理。
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}
    </div>
  );
}
