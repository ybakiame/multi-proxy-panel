import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, Spinner, Table } from "@heroui/react";
import { PageHeader } from "../components/ui";
import { formatDateTime } from "../utils/format";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { getClients } from "../api/clients";
import { getBindings } from "../api/bindings";
import { getMetrics } from "../api/metrics";
import { getOnlineCount } from "../api/onlines";
import { getLogs } from "../api/logs";
import {
  Node,
  ProtocolConfig,
  Client,
  Binding,
  Metric,
  Log,
} from "../api/types";

export function Dashboard() {
  const { t } = useTranslation();
  const [loading, setLoading] = useState(true);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [protocols, setProtocols] = useState<ProtocolConfig[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [bindings, setBindings] = useState<Binding[]>([]);
  const [metrics, setMetrics] = useState<Metric[]>([]);
  const [logs, setLogs] = useState<Log[]>([]);
  const [onlineCount, setOnlineCount] = useState(0);
  const [error, setError] = useState("");

  useEffect(() => {
    const load = async () => {
      try {
        const [
          nodesRes,
          protocolsRes,
          clientsRes,
          bindingsRes,
          metricsRes,
          logsRes,
          countRes,
        ] = await Promise.allSettled([
          getNodes(),
          getAllProtocols(),
          getClients(1, 1000),
          getBindings(),
          getMetrics(),
          getLogs(1, 5),
          getOnlineCount(),
        ]);

        if (nodesRes.status === "fulfilled") setNodes(nodesRes.value);
        if (protocolsRes.status === "fulfilled")
          setProtocols(protocolsRes.value);
        if (clientsRes.status === "fulfilled")
          setClients(clientsRes.value.data);
        if (bindingsRes.status === "fulfilled") setBindings(bindingsRes.value);
        if (metricsRes.status === "fulfilled")
          setMetrics(metricsRes.value.data);
        if (logsRes.status === "fulfilled") setLogs(logsRes.value.data);
        if (countRes.status === "fulfilled")
          setOnlineCount(countRes.value.count);

        const failed = [
          nodesRes,
          protocolsRes,
          clientsRes,
          bindingsRes,
          metricsRes,
          logsRes,
          countRes,
        ].filter((r) => r.status === "rejected").length;
        if (failed > 0) setError(t("dashboard.error"));
      } finally {
        setLoading(false);
      }
    };
    load();
  }, [t]);

  const onlineNodes = nodes.filter((n) => n.status === "online").length;

  const stats = [
    { label: t("dashboard.totalNodes"), value: nodes.length },
    { label: t("dashboard.online"), value: onlineNodes },
    { label: t("dashboard.onlineUsers"), value: onlineCount },
    { label: t("dashboard.protocols"), value: protocols.length },
    { label: t("dashboard.clients"), value: clients.length },
    { label: t("dashboard.bindings"), value: bindings.length },
    { label: t("dashboard.metricsRecords"), value: metrics.length },
  ];

  if (loading) {
    return (
      <div className="flex h-64 flex-col items-center justify-center gap-2">
        <Spinner />
        <span className="text-sm text-muted-foreground">
          {t("dashboard.loading")}
        </span>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader title={t("dashboard.title")} />

      {error && (
        <div className="rounded-lg border border-danger/30 bg-danger/10 px-4 py-2 text-sm text-danger">
          {error}
        </div>
      )}

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((s) => (
          <Card key={s.label}>
            <Card.Content className="p-5">
              <p className="text-sm text-muted-foreground">{s.label}</p>
              <p className="mt-1 text-3xl font-bold">{s.value}</p>
            </Card.Content>
          </Card>
        ))}
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Card>
          <Card.Header>
            <h3 className="text-lg font-semibold">
              {t("dashboard.recentLogs")}
            </h3>
          </Card.Header>
          <Card.Content>
            <Table aria-label="recent logs">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("logs.level")}</Table.Column>
                    <Table.Column>{t("logs.source")}</Table.Column>
                    <Table.Column>{t("logs.message")}</Table.Column>
                    <Table.Column>{t("logs.time")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("logs.empty")}
                      </div>
                    )}
                  >
                    {logs.map((log) => (
                      <Table.Row key={log.id}>
                        <Table.Cell>{log.level}</Table.Cell>
                        <Table.Cell>{log.source}</Table.Cell>
                        <Table.Cell className="max-w-xs truncate">
                          {log.message}
                        </Table.Cell>
                        <Table.Cell>
                          {formatDateTime(log.created_at)}
                        </Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          </Card.Content>
        </Card>

        <Card>
          <Card.Header>
            <h3 className="text-lg font-semibold">
              {t("dashboard.nodeStatus")}
            </h3>
          </Card.Header>
          <Card.Content>
            <Table aria-label="node status">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("nodes.name")}</Table.Column>
                    <Table.Column>{t("nodes.hostname")}</Table.Column>
                    <Table.Column>{t("nodes.address")}</Table.Column>
                    <Table.Column>{t("common.status")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </div>
                    )}
                  >
                    {nodes.map((node) => (
                      <Table.Row key={node.id}>
                        <Table.Cell>{node.name}</Table.Cell>
                        <Table.Cell>{node.hostname}</Table.Cell>
                        <Table.Cell>{node.address}</Table.Cell>
                        <Table.Cell>{node.status}</Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          </Card.Content>
        </Card>
      </div>
    </div>
  );
}
