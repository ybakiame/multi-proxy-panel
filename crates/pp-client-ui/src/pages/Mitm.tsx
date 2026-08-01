import { useEffect, useState } from "react";
import { Alert, Button, Card, TextArea } from "@heroui/react";
import { useAppStore } from "../store";

export default function Mitm() {
  const { config, traffic, loadConfig, refreshTraffic, saveConfig } = useAppStore();
  const [hostnames, setHostnames] = useState("");
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void loadConfig();
    void refreshTraffic();
  }, [loadConfig, refreshTraffic]);

  // 配置加载后同步白名单文本（每行一个 hostname）。
  useEffect(() => {
    setHostnames(config?.mitm_hostnames.join("\n") ?? "");
  }, [config?.mitm_hostnames]);

  const handleSave = async () => {
    if (!config) {
      return;
    }
    setSaving(true);
    setSaved(false);
    setError(null);
    try {
      const list = hostnames
        .split("\n")
        .map((item) => item.trim())
        .filter(Boolean);
      await saveConfig({ ...config, mitm_hostnames: list });
      setSaved(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const caDir = config ? `${config.data_dir}/certs` : "-";

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">MITM</h1>
        <p className="text-sm text-muted">中间人代理的 CA 证书、抓包白名单与记录</p>
      </div>

      <div className="grid gap-4 xl:grid-cols-2">
        <div className="flex flex-col gap-6">
          <Card>
            <Card.Header>
              <Card.Title>CA 状态</Card.Title>
              <Card.Description>证书由 Agent 内置 ACME 自动签发与管理</Card.Description>
            </Card.Header>
            <Card.Content>
              <dl className="flex flex-col gap-3 text-sm">
                <div className="flex items-center justify-between gap-4">
                  <dt className="text-muted">MITM 开关</dt>
                  <dd>{config?.mitm_enabled ? "已启用" : "未启用"}</dd>
                </div>
                <div className="flex items-center justify-between gap-4">
                  <dt className="text-muted">CA 目录</dt>
                  <dd className="truncate font-mono">{caDir}</dd>
                </div>
                <div className="flex items-center justify-between gap-4">
                  <dt className="text-muted">脚本方言</dt>
                  <dd>{config?.mitm_script_dialect ?? "-"}</dd>
                </div>
              </dl>
            </Card.Content>
          </Card>

          <Card>
            <Card.Header>
              <Card.Title>Hostname 白名单</Card.Title>
              <Card.Description>每行一个域名，仅对命中域名做中间人抓包</Card.Description>
            </Card.Header>
            <Card.Content>
              <TextArea
                aria-label="Hostname 白名单"
                value={hostnames}
                onChange={(event) => setHostnames(event.target.value)}
                placeholder={"example.com\n*.example.com"}
                rows={6}
                fullWidth
              />
            </Card.Content>
            <Card.Footer>
              <Button variant="primary" isPending={saving} onPress={() => void handleSave()}>
                保存白名单
              </Button>
              {saved && <span className="text-sm text-success">已保存</span>}
            </Card.Footer>
          </Card>

          {error && (
            <Alert status="danger">
              <Alert.Indicator />
              <Alert.Content>
                <Alert.Title>保存失败</Alert.Title>
                <Alert.Description>{error}</Alert.Description>
              </Alert.Content>
            </Alert>
          )}
        </div>

        <Card>
          <Card.Header>
            <Card.Title>抓包记录</Card.Title>
            <Card.Description>MITM 捕获的 HTTP 流量（当前为空）</Card.Description>
          </Card.Header>
          <Card.Content>
            {traffic.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
                <span className="text-sm text-muted">暂无抓包记录</span>
                <span className="text-xs text-muted/80">后端 `list_traffic` 暂未暴露 recorder，MVP 阶段恒为空列表</span>
              </div>
            ) : (
              <ul className="flex flex-col gap-2 text-sm">
                {traffic.map((record) => (
                  <li key={record.id} className="flex items-center gap-2">
                    <span className="font-mono text-muted">
                      {record.method} {record.url}
                    </span>
                    <span>{record.response_status}</span>
                  </li>
                ))}
              </ul>
            )}
          </Card.Content>
        </Card>
      </div>
    </div>
  );
}
