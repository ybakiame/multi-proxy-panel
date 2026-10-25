import { useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  ServerIcon,
  SignalIcon,
  UsersIcon,
  CogIcon,
  ArrowRightIcon,
  ArrowPathIcon,
  PlusIcon,
} from "@heroicons/react/24/outline";
import { PageHeader, TrafficChart, DataTable } from "../components/ui";
import { useHubVersion } from "../components/VersionBadge";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { StatusBadge } from "../components/ui/StatusBadge";
import { Spinner } from "../components/ui/spinner";
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import type { TrafficPoint } from "../components/ui/TrafficChart";
import { formatBytes, formatDateTime } from "../utils/format";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { getClients } from "../api/clients";
import { getOnlineCount } from "../api/onlines";
import { getLogs } from "../api/logs";
import { getTraffic } from "../api/traffic";

// Compute once at module load for the "last 24h" dashboard window.
const start24hStatic = new Date(Date.now() - 24 * 3600 * 1000).toISOString();

function SectionState({
  isLoading,
  isError,
  onRetry,
  children,
}: {
  isLoading: boolean;
  isError: boolean;
  onRetry: () => void;
  children: React.ReactNode;
}) {
  const { t } = useTranslation();
  if (isLoading) {
    return (
      <div className="flex h-32 items-center justify-center">
        <Spinner aria-label={t("common.loading")} />
      </div>
    );
  }
  if (isError) {
    return (
      <div className="flex h-32 flex-col items-center justify-center gap-2">
        <span className="text-sm text-muted-foreground">{t("dashboard.sectionError")}</span>
        <Button variant="outline" size="sm" onClick={onRetry}>
          {t("common.retry")}
        </Button>
      </div>
    );
  }
  return <>{children}</>;
}

function KpiCard({
  icon: Icon,
  label,
  value,
  to,
}: {
  icon: typeof ServerIcon;
  label: string;
  value: number;
  to: string;
}) {
  return (
    <Link to={to} className="block transition-shadow hover:shadow-(--shadow-card-hover)">
      <Card size="sm" className="h-full">
        <CardContent className="flex items-center gap-3">
          <span className="flex size-9 shrink-0 items-center justify-center rounded-lg bg-accent text-accent-foreground">
            <Icon className="h-4.5 w-4.5" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="truncate text-xs text-muted-foreground">{label}</p>
            <p className="text-2xl leading-7 font-semibold tracking-tight tabular-nums">{value}</p>
          </div>
          <ArrowRightIcon className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
        </CardContent>
      </Card>
    </Link>
  );
}

export function Dashboard() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();

  const nodesQuery = useQuery({ queryKey: ["nodes"], queryFn: getNodes });
  const protocolsQuery = useQuery({ queryKey: ["protocols"], queryFn: getAllProtocols });
  const clientsQuery = useQuery({
    queryKey: ["clients", { page: 1, perPage: 1000 }],
    queryFn: () => getClients(1, 1000),
  });
  const logsQuery = useQuery({
    queryKey: ["logs", { page: 1, perPage: 5 }],
    queryFn: () => getLogs(1, 5),
  });
  const onlineCountQuery = useQuery({
    queryKey: ["onlines", "count"],
    queryFn: getOnlineCount,
  });
  const trafficQuery = useQuery({
    queryKey: ["traffic", { start: start24hStatic, limit: 5000 }],
    queryFn: () => getTraffic({ start: start24hStatic, limit: 5000 }),
  });
  const hubHealth = useHubVersion();

  const nodes = nodesQuery.data ?? [];
  const protocols = protocolsQuery.data ?? [];
  const clients = clientsQuery.data?.data ?? [];
  const logs = logsQuery.data?.data ?? [];
  const onlineCount = onlineCountQuery.data?.count ?? 0;
  const traffic24h = trafficQuery.data?.data ?? [];

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

  const trafficTotal = useMemo(
    () => traffic24h.reduce((sum, r) => sum + r.upload_bytes + r.download_bytes, 0),
    [traffic24h],
  );

  const kpiCards = [
    { icon: ServerIcon, label: t("dashboard.totalNodes"), value: nodes.length, to: "/nodes" },
    { icon: SignalIcon, label: t("dashboard.onlineUsers"), value: onlineCount, to: "/onlines" },
    { icon: UsersIcon, label: t("dashboard.clients"), value: clients.length, to: "/clients" },
    { icon: CogIcon, label: t("dashboard.protocols"), value: protocols.length, to: "/protocols" },
  ];

  const hubStatusText = hubHealth.isError
    ? t("dashboard.hubUnreachable")
    : hubHealth.data?.status === "degraded"
      ? t("dashboard.hubDegraded")
      : t("dashboard.hubHealthy");

  return (
    <div className="space-y-5">
      <PageHeader
        title={t("dashboard.title")}
        description={t("dashboard.subtitle")}
        actions={
          <>
            <Button
              variant="outline"
              size="sm"
              onClick={() => queryClient.invalidateQueries()}
              disabled={nodesQuery.isLoading}
            >
              <ArrowPathIcon className="h-3.5 w-3.5" />
              {t("common.refresh")}
            </Button>
            <Button size="sm" render={<Link to="/nodes" />}>
              <PlusIcon className="h-3.5 w-3.5" />
              {t("dashboard.addNode")}
            </Button>
          </>
        }
      />

      {/* 重点状态卡 + KPI */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <div className="flex flex-col justify-between gap-4 rounded-xl bg-(--gradient-brand) p-5 text-primary-foreground">
          <div className="space-y-1.5">
            <p className="text-xs font-medium tracking-wider uppercase opacity-80">
              {t("dashboard.systemStatus")}
            </p>
            <p className="text-2xl font-bold tracking-tight">
              {hubHealth.isLoading ? (
                <span className="inline-block h-7 w-24 animate-pulse rounded-md bg-primary-foreground/25" />
              ) : (
                hubStatusText
              )}
            </p>
            <p className="font-mono text-xs opacity-75">
              {hubHealth.isError
                ? "Hub —"
                : t("version.badgeHubOnly", { hub: hubHealth.data?.version ?? "…" })}
            </p>
          </div>
          <div className="grid grid-cols-2 gap-3 border-t border-primary-foreground/20 pt-3">
            <Link to="/nodes" className="space-y-0.5 transition-opacity hover:opacity-80">
              <p className="text-xs opacity-75">{t("dashboard.onlineNodes")}</p>
              <p className="text-lg font-semibold tabular-nums">
                {nodesQuery.isLoading ? "…" : `${onlineNodes}/${nodes.length}`}
              </p>
            </Link>
            <Link to="/traffic" className="space-y-0.5 transition-opacity hover:opacity-80">
              <p className="text-xs opacity-75">{t("dashboard.trafficTotal")}</p>
              <p className="text-lg font-semibold tabular-nums">
                {trafficQuery.isLoading ? "…" : formatBytes(trafficTotal)}
              </p>
            </Link>
          </div>
        </div>

        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:col-span-2">
          {kpiCards.map((kpi) => (
            <KpiCard key={kpi.label} {...kpi} />
          ))}
        </div>
      </div>

      {/* 流量图 + 节点状态 */}
      <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
        <Card className="xl:col-span-2">
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle className="text-sm font-semibold">{t("dashboard.traffic24h")}</CardTitle>
            <span className="font-mono text-xs text-muted-foreground tabular-nums">
              {trafficQuery.isLoading ? "…" : formatBytes(trafficTotal)}
            </span>
          </CardHeader>
          <CardContent>
            <SectionState
              isLoading={trafficQuery.isLoading}
              isError={trafficQuery.isError}
              onRetry={() => trafficQuery.refetch()}
            >
              <TrafficChart data={trafficChartData} height={240} />
            </SectionState>
          </CardContent>
        </Card>

        <Card>
          <CardHeader className="flex-row items-center justify-between">
            <CardTitle className="text-sm font-semibold">{t("dashboard.nodeStatus")}</CardTitle>
            <Link
              to="/nodes"
              className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
            >
              {t("dashboard.viewAll")}
              <ArrowRightIcon className="h-3 w-3" />
            </Link>
          </CardHeader>
          <CardContent>
            <SectionState
              isLoading={nodesQuery.isLoading}
              isError={nodesQuery.isError}
              onRetry={() => nodesQuery.refetch()}
            >
              <ul className="divide-y divide-border">
                {nodes.length === 0 ? (
                  <li className="py-8 text-center text-sm text-muted-foreground">
                    {t("common.empty")}
                  </li>
                ) : (
                  nodes.slice(0, 6).map((node) => (
                    <li key={node.id} className="flex items-center justify-between gap-2 py-2.5">
                      <div className="min-w-0">
                        <p className="truncate text-sm font-medium">{node.name}</p>
                        <p className="truncate text-xs text-muted-foreground">
                          {node.hostname || node.address}
                        </p>
                      </div>
                      <StatusBadge status={node.status} />
                    </li>
                  ))
                )}
              </ul>
            </SectionState>
          </CardContent>
        </Card>
      </div>

      {/* 最近日志 */}
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle className="text-sm font-semibold">{t("dashboard.recentLogs")}</CardTitle>
          <Link
            to="/logs"
            className="flex items-center gap-1 text-xs text-muted-foreground transition-colors hover:text-foreground"
          >
            {t("dashboard.viewAll")}
            <ArrowRightIcon className="h-3 w-3" />
          </Link>
        </CardHeader>
        <CardContent>
          <SectionState
            isLoading={logsQuery.isLoading}
            isError={logsQuery.isError}
            onRetry={() => logsQuery.refetch()}
          >
            <DataTable
              aria-label="recent logs"
              columns={4}
              empty={logs.length === 0 ? t("logs.empty") : undefined}
              header={
                <TableRow>
                  <TableHead>{t("logs.level")}</TableHead>
                  <TableHead>{t("logs.source")}</TableHead>
                  <TableHead>{t("logs.message")}</TableHead>
                  <TableHead>{t("logs.time")}</TableHead>
                </TableRow>
              }
            >
              {logs.map((log) => (
                <TableRow key={log.id}>
                  <TableCell className="font-medium">{log.level}</TableCell>
                  <TableCell>{log.source}</TableCell>
                  <TableCell className="max-w-xs truncate">{log.message}</TableCell>
                  <TableCell className="text-muted-foreground">
                    {formatDateTime(log.created_at)}
                  </TableCell>
                </TableRow>
              ))}
            </DataTable>
          </SectionState>
        </CardContent>
      </Card>
    </div>
  );
}
