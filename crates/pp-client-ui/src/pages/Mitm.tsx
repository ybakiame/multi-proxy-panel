import { useEffect, useState } from "react";
import { Alert, Button, Card, Table, TextArea } from "@heroui/react";
import { getMitmCa, type MitmCaView } from "../api";
import { useAppStore } from "../store";

export default function Mitm() {
  const { config, traffic, loadConfig, refreshTraffic, saveConfig } = useAppStore();
  const [hostnames, setHostnames] = useState("");
  const [saved, setSaved] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mitmCa, setMitmCa] = useState<MitmCaView | null>(null);
  const [caCopied, setCaCopied] = useState(false);

  useEffect(() => {
    void loadConfig();
    void refreshTraffic();
    void getMitmCa()
      .then(setMitmCa)
      .catch((err) => setError(err instanceof Error ? err.message : String(err)));
  }, [loadConfig, refreshTraffic]);

  // 配置加载后同步白名单文本（每行一个 hostname）。
  // `mitm_hostnames` 在 config 未加载完成或字段缺失时可能为 undefined，
  // 需在 join 前防御，避免渲染期 TypeError 导致整页崩溃（黑屏）。
  useEffect(() => {
    setHostnames(config?.mitm_hostnames?.join("\n") ?? "");
  }, [config?.mitm_hostnames]);

  // RFC3339 时间戳转为本地可读时间；解析失败时原样展示。
  const formatTime = (iso: string) => {
    const date = new Date(iso);
    return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
  };

  // 复制 CA PEM 到剪贴板，成功后短暂提示。
  const handleCopyCaPem = async () => {
    if (!mitmCa) {
      return;
    }
    await navigator.clipboard.writeText(mitmCa.pem);
    setCaCopied(true);
    window.setTimeout(() => setCaCopied(false), 2000);
  };

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
              <Card.Title>MITM CA 证书</Card.Title>
              <Card.Description>解密 HTTPS 流量的根证书，需要被抓包的客户端信任</Card.Description>
            </Card.Header>
            <Card.Content>
              <div className="flex flex-col gap-3 text-sm">
                <dl className="flex flex-col gap-3">
                  <div className="flex items-center justify-between gap-4">
                    <dt className="text-muted">证书路径</dt>
                    <dd className="truncate font-mono text-xs">{mitmCa?.path ?? "-"}</dd>
                  </div>
                  <div className="flex items-center gap-3">
                    <Button variant="secondary" onPress={() => void handleCopyCaPem()}>
                      复制证书 PEM
                    </Button>
                    {caCopied && <span className="text-sm text-success">已复制到剪贴板</span>}
                  </div>
                </dl>
                <ul className="mt-1 list-inside list-disc space-y-1 text-sm text-muted">
                  <li>桌面端：双击 ca.crt 导入系统/用户信任库；Firefox 使用自带证书管理器，需单独导入</li>
                  <li>
                    Android 7+：应用默认不信任用户证书，仅安装「用户证书」对多数 App 无效；需 root 后将 CA 写入
                    /system/etc/security/cacerts/ 或目标 App 声明信任用户 CA
                  </li>
                  <li>
                    iOS：安装描述文件后，必须到「设置 → 通用 → 关于本机 →
                    证书信任设置」开启对该证书的完全信任，否则握手会被拒绝（CertificateUnknown）
                  </li>
                </ul>
              </div>
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
            <Card.Description>MITM 捕获的 HTTP 流量，仅在代理运行时可用</Card.Description>
          </Card.Header>
          <Card.Content>
            {traffic.length === 0 ? (
              <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
                <span className="text-sm text-muted">暂无抓包记录</span>
                <span className="text-xs text-muted/80">代理启动并命中 Hostname 白名单后，流量将显示在此</span>
              </div>
            ) : (
              <Table>
                <Table.ScrollContainer>
                  <Table.Content aria-label="抓包记录" className="min-w-[640px]">
                    <Table.Header>
                      <Table.Column isRowHeader>时间</Table.Column>
                      <Table.Column>方法</Table.Column>
                      <Table.Column>URL</Table.Column>
                      <Table.Column>状态</Table.Column>
                      <Table.Column>耗时</Table.Column>
                    </Table.Header>
                    <Table.Body>
                      {traffic.map((record) => (
                        <Table.Row key={record.id}>
                          <Table.Cell>{formatTime(record.timestamp)}</Table.Cell>
                          <Table.Cell>{record.method}</Table.Cell>
                          <Table.Cell>{record.url}</Table.Cell>
                          <Table.Cell>{record.response_status}</Table.Cell>
                          <Table.Cell>{record.duration_ms} ms</Table.Cell>
                        </Table.Row>
                      ))}
                    </Table.Body>
                  </Table.Content>
                </Table.ScrollContainer>
              </Table>
            )}
          </Card.Content>
          <Card.Footer>
            <Button variant="primary" onPress={() => void refreshTraffic()}>
              刷新
            </Button>
          </Card.Footer>
        </Card>
      </div>
    </div>
  );
}
