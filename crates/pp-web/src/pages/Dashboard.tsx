import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Card, CardBody, CardHeader, Spinner, Table, TableHeader, TableColumn, TableBody, TableRow, TableCell } from "@heroui/react";
import { PageHeader } from "../components/ui";
import { formatDateTime } from "../utils/format";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { getClients } from "../api/clients";
import { getBindings } from "../api/bindings";
import { getMetrics } from "../api/metrics";
import { getOnlineCount } from "../api/onlines";
import { getLogs } from "../api/logs";
import { Node, ProtocolConfig, Client, Binding, Metric, Log } from "../api/types";

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
        const [nodesRes, protocolsRes, clientsRes, bindingsRes, metricsRes, logsRes, countRes] =
          await Promise.allSettled([
            getNodes(),
            getAllProtocols(),
            getClients(1, 1000),
            getBindings(),
            getMetrics(),
            getLogs(1, 5),
            getOnlineCount(),
          ]);

        if (nodesRes.status === "fulfilled") setNodes(nodesRes.value);
        if (protocolsRes.status === "fulfilled") setProtocols(protocolsRes.value);
        if (clientsRes.status === "fulfilled") setClients(clientsRes.value.data);
        if (bindingsRes.status === "fulfilled") setBindings(bindingsRes.value);
        if (metricsRes.status === "fulfilled") setMetrics(metricsRes.value.data);
        if (logsRes.status === "fulfilled") setLogs(logsRes.value.data);
        if (countRes.status === "fulfilled") setOnlineCount(countRes.value.count);

        const failed = [nodesRes, protocolsRes, clientsRes, bindingsRes, metricsRes, logsRes, countRes]
          .filter((r) => r.status === "rejected").length;
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
      <div className="flex h-64 items-center justify-center">
        <Spinner label={t("dashboard.loading")} />
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
            <CardBody className="p-5">
              <p className="text-sm text-muted-foreground">{s.label}</p>
              <p className="mt-1 text-3xl font-bold">{s.value}</p>
            </CardBody>
          </Card>
        ))}
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <h3 className="text-lg font-semibold">{t("dashboard.recentLogs")}</h3>
          </CardHeader>
          <CardBody>
            <Table removeWrapper aria-label="recent logs">
              <TableHeader>
                <TableColumn>{t("logs.level")}</TableColumn>
                <TableColumn>{t("logs.source")}</TableColumn>
                <TableColumn>{t("logs.message")}</TableColumn>
                <TableColumn>{t("logs.time")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("logs.empty")}>
                {logs.map((log) => (
                  <TableRow key={log.id}>
                    <TableCell>{log.level}</TableCell>
                    <TableCell>{log.source}</TableCell>
                    <TableCell className="max-w-xs truncate">{log.message}</TableCell>
                    <TableCell>{formatDateTime(log.created_at)}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardBody>
        </Card>

        <Card>
          <CardHeader>
            <h3 className="text-lg font-semibold">{t("dashboard.nodeStatus")}</h3>
          </CardHeader>
          <CardBody>
            <Table removeWrapper aria-label="node status">
              <TableHeader>
                <TableColumn>{t("nodes.name")}</TableColumn>
                <TableColumn>{t("nodes.hostname")}</TableColumn>
                <TableColumn>{t("nodes.address")}</TableColumn>
                <TableColumn>{t("common.status")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {nodes.map((node) => (
                  <TableRow key={node.id}>
                    <TableCell>{node.name}</TableCell>
                    <TableCell>{node.hostname}</TableCell>
                    <TableCell>{node.address}</TableCell>
                    <TableCell>{node.status}</TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </CardBody>
        </Card>
      </div>
    </div>
  );
}
