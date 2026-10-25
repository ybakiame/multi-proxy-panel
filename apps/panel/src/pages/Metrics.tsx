import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import {
  PageHeader,
  Pagination,
  FormSelect,
  FormCheckbox,
  MetricsChart,
  DataTable,
} from "../components/ui";
import { Button } from "../components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "../components/ui/card";
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import { usePagination } from "../hooks/useCommon";
import { getMetrics } from "../api/metrics";
import { getNodes } from "../api/nodes";
import { formatBytes, formatDateTime } from "../utils/format";

const metricsQueryKey = "metrics";
const nodesQueryKey = "nodes";

export function Metrics() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage } = usePagination(1, 20);
  const [nodeId, setNodeId] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(false);

  const {
    data: metricsData,
    isLoading,
    refetch,
  } = useQuery({
    queryKey: [metricsQueryKey, { nodeId }],
    queryFn: () => getMetrics(nodeId || undefined),
    refetchInterval: autoRefresh ? 30000 : false,
  });

  const metrics = metricsData?.data ?? [];
  const total = metricsData?.pagination?.total ?? metrics.length ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const display = metrics.slice((page - 1) * perPage, page * perPage);

  const nodeOptions = [
    { id: "", label: t("common.all") },
    ...nodes.map((node) => ({ id: node.id, label: node.name })),
  ];

  return (
    <div className="space-y-4">
      <PageHeader title={t("metrics.title")} />
      <div className="flex flex-wrap items-end gap-4">
        <FormSelect
          label={t("metrics.filterByNode")}
          value={nodeId}
          onChange={(value) => {
            setNodeId(value);
            setPage(1);
          }}
          options={nodeOptions}
          className="min-w-[240px]"
        />
        <FormCheckbox isSelected={autoRefresh} onChange={setAutoRefresh}>
          {t("metrics.autoRefresh")}
        </FormCheckbox>
        <Button onClick={() => refetch()}>{t("common.refresh")}</Button>
      </div>
      <Card>
        <CardHeader>
          <CardTitle className="text-lg font-semibold">{t("metrics.chartTitle")}</CardTitle>
        </CardHeader>
        <CardContent>
          <MetricsChart metrics={metrics} />
        </CardContent>
      </Card>
      <Card>
        <CardContent>
          <DataTable
            aria-label="metrics"
            columns={8}
            loading={isLoading}
            empty={display.length === 0 ? t("metrics.empty") : undefined}
            header={
              <TableRow>
                <TableHead>{t("nodes.name")}</TableHead>
                <TableHead>{t("metrics.cpu")}</TableHead>
                <TableHead>{t("metrics.memory")}</TableHead>
                <TableHead>{t("metrics.disk")}</TableHead>
                <TableHead>{t("metrics.netRx")}</TableHead>
                <TableHead>{t("metrics.netTx")}</TableHead>
                <TableHead>{t("metrics.loadAvg")}</TableHead>
                <TableHead>{t("metrics.timestamp")}</TableHead>
              </TableRow>
            }
          >
            {display.map((m) => {
              const node = nodes.find((n) => n.id === m.node_id);
              return (
                <TableRow key={m.id}>
                  <TableCell>{node ? node.name : m.node_id}</TableCell>
                  <TableCell>{(m.cpu_percent ?? 0).toFixed(2)}%</TableCell>
                  <TableCell>
                    {formatBytes(m.mem_used ?? 0)} / {formatBytes(m.mem_total ?? 0)}
                  </TableCell>
                  <TableCell>
                    {formatBytes(m.disk_used ?? 0)} / {formatBytes(m.disk_total ?? 0)}
                  </TableCell>
                  <TableCell>{formatBytes(m.net_rx ?? 0)}</TableCell>
                  <TableCell>{formatBytes(m.net_tx ?? 0)}</TableCell>
                  <TableCell>
                    {(m.load_avg1 ?? 0).toFixed(2)} / {(m.load_avg5 ?? 0).toFixed(2)} /{" "}
                    {(m.load_avg15 ?? 0).toFixed(2)}
                  </TableCell>
                  <TableCell>{formatDateTime(m.timestamp)}</TableCell>
                </TableRow>
              );
            })}
          </DataTable>
          <Pagination
            page={page}
            totalPages={totalPages}
            perPage={perPage}
            total={total}
            onPageChange={setPage}
            onPerPageChange={setPerPage}
          />
        </CardContent>
      </Card>
    </div>
  );
}
