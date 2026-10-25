import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { PageHeader, TrafficChart } from "../components/ui";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { Spinner } from "../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import type { TrafficPoint } from "../components/ui/TrafficChart";
import { formatDateTime } from "../utils/format";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { getClients } from "../api/clients";
import { getBindings } from "../api/bindings";
import { getMetrics } from "../api/metrics";
import { getOnlineCount } from "../api/onlines";
import { getLogs } from "../api/logs";
import { getTraffic } from "../api/traffic";

// Compute once at module load for the "last 24h" dashboard window.
const start24hStatic = new Date(Date.now() - 24 * 3600 * 1000).toISOString();

export function Dashboard() {
  const { t } = useTranslation();

  const { data: nodes = [], isLoading: nodesLoading } = useQuery({
    queryKey: ["nodes"],
    queryFn: getNodes,
  });

  const { data: protocols = [], isLoading: protocolsLoading } = useQuery({
    queryKey: ["protocols"],
    queryFn: getAllProtocols,
  });

  const { data: clientsData, isLoading: clientsLoading } = useQuery({
    queryKey: ["clients", { page: 1, perPage: 1000 }],
    queryFn: () => getClients(1, 1000),
  });

  const { data: bindings = [], isLoading: bindingsLoading } = useQuery({
    queryKey: ["bindings"],
    queryFn: getBindings,
  });

  const { data: metricsData, isLoading: metricsLoading } = useQuery({
    queryKey: ["metrics"],
    queryFn: () => getMetrics(),
  });

  const { data: logsData, isLoading: logsLoading } = useQuery({
    queryKey: ["logs", { page: 1, perPage: 5 }],
    queryFn: () => getLogs(1, 5),
  });

  const { data: onlineCountData, isLoading: onlineLoading } = useQuery({
    queryKey: ["onlines", "count"],
    queryFn: getOnlineCount,
  });

  const { data: trafficData, isLoading: trafficLoading } = useQuery({
    queryKey: ["traffic", { start: start24hStatic, limit: 5000 }],
    queryFn: () => getTraffic({ start: start24hStatic, limit: 5000 }),
  });

  const loading =
    nodesLoading ||
    protocolsLoading ||
    clientsLoading ||
    bindingsLoading ||
    metricsLoading ||
    logsLoading ||
    onlineLoading ||
    trafficLoading;

  const clients = clientsData?.data ?? [];
  const metrics = metricsData?.data ?? [];
  const logs = logsData?.data ?? [];
  const onlineCount = onlineCountData?.count ?? 0;
  const traffic24h = trafficData?.data ?? [];

  const onlineNodes = nodes.filter((n) => n.status === "online").length;

  const trafficChartData = useMemo<TrafficPoint[]>(() => {
    const byHour = new Map<string, TrafficPoint>();
    for (const r of traffic24h) {
      const point = byHour.get(r.hour_bucket) || {
        time: new Date(r.hour_bucket).toLocaleString(undefined, {
          month: "numeric",
          day: "numeric",
          hour: "2-digit",
        }),
        upload: 0,
        download: 0,
      };
      point.upload += r.upload_bytes;
      point.download += r.download_bytes;
      byHour.set(r.hour_bucket, point);
    }
    return Array.from(byHour.entries())
      .sort(([a], [b]) => new Date(a).getTime() - new Date(b).getTime())
      .map(([, point]) => point);
  }, [traffic24h]);

  const stats = [
    { label: t("dashboard.totalNodes"), value: nodes.length, to: "/nodes" },
    { label: t("dashboard.online"), value: onlineNodes, to: "/nodes" },
    { label: t("dashboard.onlineUsers"), value: onlineCount, to: "/onlines" },
    { label: t("dashboard.protocols"), value: protocols.length, to: "/protocols" },
    { label: t("dashboard.clients"), value: clients.length, to: "/clients" },
    { label: t("dashboard.bindings"), value: bindings.length, to: "/bindings" },
    { label: t("dashboard.metricsRecords"), value: metrics.length, to: "/metrics" },
  ];

  if (loading) {
    return (
      <div className="flex h-64 flex-col items-center justify-center gap-2">
        <Spinner />
        <span className="text-sm text-muted-foreground">{t("dashboard.loading")}</span>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <PageHeader title={t("dashboard.title")} />

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {stats.map((s) => (
          <Link key={s.label} to={s.to} className="block transition-opacity hover:opacity-80">
            <Card className="h-full">
              <CardContent className="p-5">
                <p className="text-sm text-muted-foreground">{s.label}</p>
                <p className="mt-1 text-3xl font-bold">{s.value}</p>
              </CardContent>
            </Card>
          </Link>
        ))}
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="text-lg font-semibold">{t("dashboard.traffic24h")}</CardTitle>
        </CardHeader>
        <CardContent>
          <TrafficChart data={trafficChartData} height={220} />
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="text-lg font-semibold">{t("dashboard.recentLogs")}</CardTitle>
          </CardHeader>
          <CardContent>
            <Table aria-label="recent logs">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("logs.level")}</TableHead>
                  <TableHead>{t("logs.source")}</TableHead>
                  <TableHead>{t("logs.message")}</TableHead>
                  <TableHead>{t("logs.time")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {logs.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={4} className="p-4 text-center text-muted-foreground">
                      {t("logs.empty")}
                    </TableCell>
                  </TableRow>
                ) : (
                  logs.map((log) => (
                    <TableRow key={log.id}>
                      <TableCell>{log.level}</TableCell>
                      <TableCell>{log.source}</TableCell>
                      <TableCell className="max-w-xs truncate">{log.message}</TableCell>
                      <TableCell>{formatDateTime(log.created_at)}</TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="text-lg font-semibold">{t("dashboard.nodeStatus")}</CardTitle>
          </CardHeader>
          <CardContent>
            <Table aria-label="node status">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("nodes.name")}</TableHead>
                  <TableHead>{t("nodes.hostname")}</TableHead>
                  <TableHead>{t("nodes.address")}</TableHead>
                  <TableHead>{t("common.status")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {nodes.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={4} className="p-4 text-center text-muted-foreground">
                      {t("common.empty")}
                    </TableCell>
                  </TableRow>
                ) : (
                  nodes.map((node) => (
                    <TableRow key={node.id}>
                      <TableCell>{node.name}</TableCell>
                      <TableCell>{node.hostname}</TableCell>
                      <TableCell>{node.address}</TableCell>
                      <TableCell>{node.status}</TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
