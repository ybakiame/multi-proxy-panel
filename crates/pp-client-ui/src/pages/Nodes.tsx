import { useCallback, useEffect, useState } from "react";
import { Alert, Button, Card, Input, Label, Meter, Modal, Switch, Table } from "@heroui/react";
import {
  addSubscription,
  listSubscriptions,
  refreshSubscription,
  removeSubscription,
  setSubscriptionEnabled,
  toErrorMessage,
} from "../api";
import type { SubscriptionUserInfo, SubscriptionView } from "../api";

/** 字节数格式化为 GB（保留两位小数）。 */
function formatGb(bytes: number | null | undefined): string {
  if (bytes == null) {
    return "-";
  }
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}

/** 已用流量 = download + upload；total 缺失时仅显示已用。 */
function usageText(info: SubscriptionUserInfo | null): string {
  if (!info) {
    return "-";
  }
  const used = (info.download ?? 0) + (info.upload ?? 0);
  if (info.total == null) {
    return formatGb(used);
  }
  return `${formatGb(used)} / ${formatGb(info.total)}`;
}

/** 用量百分比（0-100，total 缺失时按 0 处理）。 */
function usagePercent(info: SubscriptionUserInfo | null): number {
  if (!info || info.total == null || info.total <= 0) {
    return 0;
  }
  const used = (info.download ?? 0) + (info.upload ?? 0);
  return Math.min(100, Math.max(0, (used / info.total) * 100));
}

/** 到期时间戳（秒）转为本地日期；缺失返回占位符。 */
function formatExpire(expire: number | null | undefined): string {
  if (expire == null) {
    return "-";
  }
  const date = new Date(expire * 1000);
  return Number.isNaN(date.getTime()) ? "-" : date.toLocaleDateString();
}

/** 用量进度条颜色：超量红、>80% 黄、其余强调色。 */
function usageColor(percent: number): "accent" | "warning" | "danger" {
  if (percent >= 100) {
    return "danger";
  }
  if (percent >= 80) {
    return "warning";
  }
  return "accent";
}

/** 常用 UA 快捷选择（空串 = 默认 clash.meta）。 */
const UA_PRESETS = [
  { value: "", label: "默认" },
  { value: "clash.meta", label: "clash.meta" },
  { value: "clash-verge", label: "clash-verge" },
  { value: "sing-box", label: "sing-box" },
];

type OpResult = { sub: SubscriptionView; kind: "add" | "refresh" };

export default function Nodes() {
  const [subs, setSubs] = useState<SubscriptionView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<OpResult | null>(null);
  const [refreshingId, setRefreshingId] = useState<string | null>(null);

  // 添加订阅对话框
  const [addOpen, setAddOpen] = useState(false);
  const [newName, setNewName] = useState("");
  const [newUrl, setNewUrl] = useState("");
  const [newUa, setNewUa] = useState("");

  const refreshSubs = useCallback(async () => {
    try {
      setSubs(await listSubscriptions());
      setError(null);
    } catch (err) {
      setError(toErrorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refreshSubs();
  }, [refreshSubs]);

  const handleAdd = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const ua = newUa.trim();
      const sub = await addSubscription({
        name: newName.trim(),
        url: newUrl.trim(),
        user_agent: ua || undefined,
      });
      setResult({ sub, kind: "add" });
      setAddOpen(false);
      setNewName("");
      setNewUrl("");
      setNewUa("");
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRemove = async (id: string) => {
    setBusy(true);
    setError(null);
    try {
      await removeSubscription(id);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleToggle = async (sub: SubscriptionView) => {
    setBusy(true);
    setError(null);
    try {
      await setSubscriptionEnabled(sub.id, !sub.enabled);
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const handleRefresh = async (id: string) => {
    setRefreshingId(id);
    setError(null);
    setResult(null);
    try {
      const sub = await refreshSubscription(id);
      setResult({ sub, kind: "refresh" });
      await refreshSubs();
    } catch (err) {
      setError(toErrorMessage(err));
    } finally {
      setRefreshingId(null);
    }
  };

  const anyEnabled = subs.some((sub) => sub.enabled);
  const formValid = newName.trim().length > 0 && newUrl.trim().length > 0;

  return (
    <div className="flex flex-col gap-6">
      <div>
        <h1 className="text-xl font-semibold">订阅</h1>
        <p className="text-sm text-muted">多订阅源管理：拉取节点、用量与到期展示，取第一个启用的订阅生效</p>
      </div>

      <Card>
        <Card.Header>
          <Card.Title>订阅列表</Card.Title>
          <Card.Description>添加后立即拉取一次；节点数 / 用量 / 到期为最近一次拉取结果</Card.Description>
        </Card.Header>
        <Card.Content>
          {subs.length === 0 ? (
            <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
              <span className="text-sm text-muted">暂无订阅</span>
              <span className="text-xs text-muted/80">点击「添加订阅」添加首个订阅源，启用后重启代理应用生效</span>
            </div>
          ) : (
            <Table>
              <Table.ScrollContainer>
                <Table.Content aria-label="订阅列表" className="min-w-[880px]">
                  <Table.Header>
                    <Table.Column isRowHeader>名称</Table.Column>
                    <Table.Column>URL</Table.Column>
                    <Table.Column>节点数</Table.Column>
                    <Table.Column>用量</Table.Column>
                    <Table.Column>到期时间</Table.Column>
                    <Table.Column>启用</Table.Column>
                    <Table.Column>操作</Table.Column>
                  </Table.Header>
                  <Table.Body>
                    {subs.map((sub) => {
                      const percent = usagePercent(sub.userinfo);
                      return (
                        <Table.Row key={sub.id}>
                          <Table.Cell>{sub.name}</Table.Cell>
                          <Table.Cell className="max-w-[240px] truncate font-mono text-xs">{sub.url}</Table.Cell>
                          <Table.Cell>{sub.node_count > 0 ? sub.node_count : "-"}</Table.Cell>
                          <Table.Cell className="min-w-[180px]">
                            <Meter
                              aria-label={`${sub.name} 流量用量`}
                              value={percent}
                              size="sm"
                              color={usageColor(percent)}
                              valueLabel={usageText(sub.userinfo)}
                              className="w-full"
                            >
                              <Meter.Output />
                              <Meter.Track>
                                <Meter.Fill />
                              </Meter.Track>
                            </Meter>
                          </Table.Cell>
                          <Table.Cell>{formatExpire(sub.userinfo?.expire)}</Table.Cell>
                          <Table.Cell>
                            <Switch
                              aria-label={`启用 ${sub.name}`}
                              isSelected={sub.enabled}
                              isDisabled={busy}
                              onChange={() => void handleToggle(sub)}
                            >
                              <Switch.Content>
                                <Switch.Control>
                                  <Switch.Thumb />
                                </Switch.Control>
                                <span className="sr-only">{sub.enabled ? "启用" : "停用"}</span>
                              </Switch.Content>
                            </Switch>
                          </Table.Cell>
                          <Table.Cell>
                            <div className="flex items-center gap-2">
                              <Button
                                size="sm"
                                variant="secondary"
                                isDisabled={busy}
                                isPending={refreshingId === sub.id}
                                onPress={() => void handleRefresh(sub.id)}
                              >
                                刷新
                              </Button>
                              <Button
                                size="sm"
                                variant="tertiary"
                                isDisabled={busy}
                                onPress={() => void handleRemove(sub.id)}
                              >
                                删除
                              </Button>
                            </div>
                          </Table.Cell>
                        </Table.Row>
                      );
                    })}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          )}
        </Card.Content>
        <Card.Footer>
          <div className="flex w-full items-center justify-between gap-3">
            <Button variant="secondary" isDisabled={busy} onPress={() => void refreshSubs()}>
              刷新列表
            </Button>
            <Button variant="primary" isDisabled={busy} onPress={() => setAddOpen(true)}>
              添加订阅
            </Button>
          </div>
        </Card.Footer>
      </Card>

      {anyEnabled && (
        <Alert status="accent">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>生效提示</Alert.Title>
            <Alert.Description>取第一个启用的订阅生效，重启代理应用后应用新订阅</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {result && (
        <Alert status={result.sub.error ? "warning" : "success"}>
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>
              {result.kind === "add"
                ? result.sub.error
                  ? "订阅已添加，但首次拉取失败"
                  : "订阅已添加"
                : result.sub.error
                  ? "刷新失败，已保留上次数据"
                  : "订阅已更新"}
            </Alert.Title>
            <Alert.Description>
              {result.sub.error
                ? `「${result.sub.name}」${result.sub.error}`
                : `「${result.sub.name}」共 ${result.sub.node_count} 个节点`}
            </Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {error && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description>{error}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      <Modal.Backdrop isOpen={addOpen} onOpenChange={setAddOpen}>
        <Modal.Container>
          <Modal.Dialog className="sm:max-w-[480px]">
            <Modal.CloseTrigger />
            <Modal.Header>
              <Modal.Heading>添加订阅</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="flex flex-col gap-4">
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-name">名称</Label>
                <Input
                  id="sub-name"
                  aria-label="订阅名称"
                  value={newName}
                  onChange={(event) => setNewName(event.target.value)}
                  placeholder="我的机场"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-url">URL</Label>
                <Input
                  id="sub-url"
                  aria-label="订阅 URL"
                  value={newUrl}
                  onChange={(event) => setNewUrl(event.target.value)}
                  placeholder="https://example.com/sub"
                  fullWidth
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label htmlFor="sub-ua">User-Agent（可选）</Label>
                <Input
                  id="sub-ua"
                  aria-label="订阅 User-Agent"
                  value={newUa}
                  onChange={(event) => setNewUa(event.target.value)}
                  placeholder="留空使用默认 clash.meta"
                  fullWidth
                />
                <div className="flex flex-wrap items-center gap-1.5 pt-1">
                  <span className="text-xs text-muted">常用：</span>
                  {UA_PRESETS.map((preset) => (
                    <Button
                      key={preset.value}
                      size="sm"
                      variant={newUa === preset.value ? "primary" : "secondary"}
                      onPress={() => setNewUa(preset.value)}
                    >
                      {preset.label}
                    </Button>
                  ))}
                </div>
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="secondary" onPress={() => setAddOpen(false)}>
                取消
              </Button>
              <Button variant="primary" isPending={busy} isDisabled={!formValid} onPress={() => void handleAdd()}>
                添加
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
