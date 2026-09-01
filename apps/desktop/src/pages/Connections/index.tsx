import { useCallback, useRef, useState } from "react";
import { Alert, Button, Card, Chip, Spinner, Table } from "@heroui/react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { connectionsActive, connectionsClosed, connectionsClose, toErrorMessage } from "../../api";
import type { ConnectionView } from "../../api";
import { toastError, toastSuccess } from "../../toast";
import { useAppStore } from "../../store";

const CONNECTIONS_KEY = ["connections_active"];
const CLOSED_KEY = ["connections_closed"];
const REFETCH_INTERVAL_MS = 2000;

/** Format bytes to human-readable string (B / KB / MB / GB). */
function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const k = 1024;
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), units.length - 1);
  return `${(bytes / k ** i).toFixed(i === 0 ? 0 : 2)} ${units[i]}`;
}

/** Calculate duration from start timestamp to now, returns "HH:MM:SS" or "MM:SS". */
function formatDuration(start: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = Math.max(0, now - start);
  const h = Math.floor(diff / 3600);
  const m = Math.floor((diff % 3600) / 60);
  const s = diff % 60;
  if (h > 0) {
    return `${h.toString().padStart(2, "0")}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  }
  return `${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
}

/** Network type chip color. */
function networkColor(network: string): "default" | "success" | "warning" {
  switch (network.toLowerCase()) {
    case "tcp":
      return "success";
    case "udp":
      return "warning";
    default:
      return "default";
  }
}

export default function Connections() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const status = useAppStore((s) => s.status);
  const _running = status?.core_running ?? false;

  const [actionError, setActionError] = useState<string | null>(null);
  const [closingId, setClosingId] = useState<string | null>(null);
  const [showClosed, setShowClosed] = useState(false);

  // Skip polling during close action
  const skipPollRef = useRef(false);

  const {
    data: activeData,
    isLoading: activeLoading,
    error: activeError,
  } = useQuery<{ connections: ConnectionView[]; upload_total: number; download_total: number }>({
    queryKey: CONNECTIONS_KEY,
    queryFn: connectionsActive,
    refetchInterval: (query) => {
      if (skipPollRef.current) return false;
      if (query.state.error) return 10000;
      return REFETCH_INTERVAL_MS;
    },
    retry: false,
  });

  const { data: closedData, isLoading: closedLoading } = useQuery<ConnectionView[]>({
    queryKey: CLOSED_KEY,
    queryFn: connectionsClosed,
    refetchInterval: (query) => {
      if (skipPollRef.current) return false;
      if (query.state.error) return 10000;
      return REFETCH_INTERVAL_MS;
    },
    retry: false,
    enabled: showClosed,
  });

  const coreNotRunning = Boolean(activeError);

  const handleClose = useCallback(
    async (id: string) => {
      setClosingId(id);
      skipPollRef.current = true;
      setActionError(null);
      try {
        await connectionsClose(id);
        await queryClient.invalidateQueries({ queryKey: CONNECTIONS_KEY });
        toastSuccess("连接已关闭");
      } catch (err) {
        const msg = toErrorMessage(err);
        setActionError(msg);
        toastError(msg);
      } finally {
        setClosingId(null);
        window.setTimeout(() => {
          skipPollRef.current = false;
        }, 1500);
      }
    },
    [queryClient],
  );

  return (
    <div className="flex flex-col gap-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-xl font-semibold">连接</h1>
          <p className="text-sm text-muted">当前活跃连接与已关闭记录</p>
        </div>
      </div>

      {actionError && (
        <Alert status="danger">
          <Alert.Indicator />
          <Alert.Content>
            <Alert.Title>操作失败</Alert.Title>
            <Alert.Description className="break-all">{actionError}</Alert.Description>
          </Alert.Content>
        </Alert>
      )}

      {coreNotRunning && (
        <Card>
          <Card.Content className="flex flex-col items-center justify-center gap-4 py-12 text-center">
            <span className="text-sm text-muted">启动代理后可用</span>
            <span className="text-xs text-muted/80">代理核心未运行，无法获取连接信息</span>
            <Button variant="primary" onPress={() => navigate("/")}>
              前往首页
            </Button>
          </Card.Content>
        </Card>
      )}

      {activeLoading && !coreNotRunning && (
        <div className="flex items-center justify-center py-12">
          <Spinner />
        </div>
      )}

      {!coreNotRunning && activeData && (
        <>
          {/* Totals */}
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
            <Card>
              <Card.Header>
                <Card.Title>活跃连接</Card.Title>
              </Card.Header>
              <Card.Content>
                <span className="text-lg font-semibold">{activeData.connections.length}</span>
              </Card.Content>
            </Card>
            <Card>
              <Card.Header>
                <Card.Title>总上行</Card.Title>
              </Card.Header>
              <Card.Content>
                <span className="text-lg font-semibold">{formatBytes(activeData.upload_total)}</span>
              </Card.Content>
            </Card>
            <Card>
              <Card.Header>
                <Card.Title>总下行</Card.Title>
              </Card.Header>
              <Card.Content>
                <span className="text-lg font-semibold">{formatBytes(activeData.download_total)}</span>
              </Card.Content>
            </Card>
          </div>

          {/* Active connections table */}
          <Card>
            <Card.Header>
              <Card.Title>活跃连接</Card.Title>
              <Card.Description>实时连接列表（每 2 秒刷新）</Card.Description>
            </Card.Header>
            <Card.Content>
              {activeData.connections.length === 0 ? (
                <div className="flex flex-col items-center justify-center gap-2 py-12 text-center">
                  <span className="text-sm text-muted">暂无活跃连接</span>
                  <span className="text-xs text-muted/80">启动代理并产生流量后将显示连接</span>
                </div>
              ) : (
                <Table>
                  <Table.ScrollContainer>
                    <Table.Content aria-label="活跃连接列表" className="min-w-[960px]">
                      <Table.Header>
                        <Table.Column isRowHeader>目标</Table.Column>
                        <Table.Column>协议</Table.Column>
                        <Table.Column>链</Table.Column>
                        <Table.Column>规则</Table.Column>
                        <Table.Column>上行</Table.Column>
                        <Table.Column>下行</Table.Column>
                        <Table.Column>时长</Table.Column>
                        <Table.Column>操作</Table.Column>
                      </Table.Header>
                      <Table.Body>
                        {activeData.connections.map((conn) => (
                          <Table.Row key={conn.id}>
                            <Table.Cell className="max-w-[200px] truncate">
                              <span title={conn.host}>{conn.host}</span>
                            </Table.Cell>
                            <Table.Cell>
                              <Chip size="sm" variant="soft" color={networkColor(conn.network)}>
                                {conn.network.toUpperCase()}
                              </Chip>
                            </Table.Cell>
                            <Table.Cell className="max-w-[240px] truncate text-xs">
                              <span title={conn.chain}>{conn.chain || "-"}</span>
                            </Table.Cell>
                            <Table.Cell className="max-w-[160px] truncate text-xs">
                              <span title={`${conn.rule}${conn.rule_payload ? `: ${conn.rule_payload}` : ""}`}>
                                {conn.rule}
                                {conn.rule_payload ? ` (${conn.rule_payload})` : ""}
                              </span>
                            </Table.Cell>
                            <Table.Cell className="text-xs">{formatBytes(conn.upload)}</Table.Cell>
                            <Table.Cell className="text-xs">{formatBytes(conn.download)}</Table.Cell>
                            <Table.Cell className="text-xs font-mono">{formatDuration(conn.start)}</Table.Cell>
                            <Table.Cell>
                              <Button
                                size="sm"
                                variant="tertiary"
                                isPending={closingId === conn.id}
                                isDisabled={closingId !== null}
                                onPress={() => void handleClose(conn.id)}
                              >
                                关闭
                              </Button>
                            </Table.Cell>
                          </Table.Row>
                        ))}
                      </Table.Body>
                    </Table.Content>
                  </Table.ScrollContainer>
                </Table>
              )}
            </Card.Content>
          </Card>
        </>
      )}

      {/* Closed connections (collapsible) */}
      {!coreNotRunning && (
        <Card>
          <Card.Header className="cursor-pointer" onClick={() => setShowClosed((prev) => !prev)}>
            <div className="flex items-center gap-2">
              <Card.Title>已关闭记录</Card.Title>
              <Chip size="sm" variant="soft" color="default">
                {closedData?.length ?? 0}
              </Chip>
            </div>
            <Card.Description>点击{showClosed ? "收起" : "展开"}已关闭连接记录</Card.Description>
          </Card.Header>
          {showClosed && (
            <Card.Content>
              {closedLoading && (
                <div className="flex items-center justify-center py-8">
                  <Spinner />
                </div>
              )}
              {!closedLoading && (!closedData || closedData.length === 0) && (
                <div className="flex flex-col items-center justify-center gap-2 py-8 text-center">
                  <span className="text-sm text-muted">暂无已关闭记录</span>
                  <span className="text-xs text-muted/80">连接关闭后将在此显示</span>
                </div>
              )}
              {!closedLoading && closedData && closedData.length > 0 && (
                <Table>
                  <Table.ScrollContainer>
                    <Table.Content aria-label="已关闭连接列表" className="min-w-[960px]">
                      <Table.Header>
                        <Table.Column isRowHeader>目标</Table.Column>
                        <Table.Column>协议</Table.Column>
                        <Table.Column>链</Table.Column>
                        <Table.Column>规则</Table.Column>
                        <Table.Column>上行</Table.Column>
                        <Table.Column>下行</Table.Column>
                      </Table.Header>
                      <Table.Body>
                        {closedData.map((conn) => (
                          <Table.Row key={conn.id}>
                            <Table.Cell className="max-w-[200px] truncate">
                              <span title={conn.host}>{conn.host}</span>
                            </Table.Cell>
                            <Table.Cell>
                              <Chip size="sm" variant="soft" color={networkColor(conn.network)}>
                                {conn.network.toUpperCase()}
                              </Chip>
                            </Table.Cell>
                            <Table.Cell className="max-w-[240px] truncate text-xs">
                              <span title={conn.chain}>{conn.chain || "-"}</span>
                            </Table.Cell>
                            <Table.Cell className="max-w-[160px] truncate text-xs">
                              <span title={`${conn.rule}${conn.rule_payload ? `: ${conn.rule_payload}` : ""}`}>
                                {conn.rule}
                                {conn.rule_payload ? ` (${conn.rule_payload})` : ""}
                              </span>
                            </Table.Cell>
                            <Table.Cell className="text-xs">{formatBytes(conn.upload)}</Table.Cell>
                            <Table.Cell className="text-xs">{formatBytes(conn.download)}</Table.Cell>
                          </Table.Row>
                        ))}
                      </Table.Body>
                    </Table.Content>
                  </Table.ScrollContainer>
                </Table>
              )}
            </Card.Content>
          )}
        </Card>
      )}
    </div>
  );
}
