import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { Button, Card, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination, FormSelect, FormCheckbox, MetricsChart } from "../components/ui";
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
        <Button onPress={() => refetch()}>{t("common.refresh")}</Button>
      </div>
      <Card>
        <Card.Header>
          <h3 className="text-lg font-semibold">{t("metrics.chartTitle")}</h3>
        </Card.Header>
        <Card.Content>
          <MetricsChart metrics={metrics} />
        </Card.Content>
      </Card>
      <Card>
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="metrics">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("nodes.name")}</Table.Column>
                      <Table.Column>{t("metrics.cpu")}</Table.Column>
                      <Table.Column>{t("metrics.memory")}</Table.Column>
                      <Table.Column>{t("metrics.disk")}</Table.Column>
                      <Table.Column>{t("metrics.netRx")}</Table.Column>
                      <Table.Column>{t("metrics.netTx")}</Table.Column>
                      <Table.Column>{t("metrics.loadAvg")}</Table.Column>
                      <Table.Column>{t("metrics.timestamp")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("metrics.empty")}
                        </div>
                      )}
                    >
                      {display.map((m) => {
                        const node = nodes.find((n) => n.id === m.node_id);
                        return (
                          <Table.Row key={m.id}>
                            <Table.Cell>{node ? node.name : m.node_id}</Table.Cell>
                            <Table.Cell>{(m.cpu_percent ?? 0).toFixed(2)}%</Table.Cell>
                            <Table.Cell>
                              {formatBytes(m.mem_used ?? 0)} / {formatBytes(m.mem_total ?? 0)}
                            </Table.Cell>
                            <Table.Cell>
                              {formatBytes(m.disk_used ?? 0)} / {formatBytes(m.disk_total ?? 0)}
                            </Table.Cell>
                            <Table.Cell>{formatBytes(m.net_rx ?? 0)}</Table.Cell>
                            <Table.Cell>{formatBytes(m.net_tx ?? 0)}</Table.Cell>
                            <Table.Cell>
                              {(m.load_avg1 ?? 0).toFixed(2)} / {(m.load_avg5 ?? 0).toFixed(2)} /{" "}
                              {(m.load_avg15 ?? 0).toFixed(2)}
                            </Table.Cell>
                            <Table.Cell>{formatDateTime(m.timestamp)}</Table.Cell>
                          </Table.Row>
                        );
                      })}
                    </Table.Body>
                  </Table.Content>
                </Table.ScrollContainer>
              </Table>
              <Pagination
                page={page}
                totalPages={totalPages}
                perPage={perPage}
                total={total}
                onPageChange={setPage}
                onPerPageChange={setPerPage}
              />
            </>
          )}
        </Card.Content>
      </Card>
    </div>
  );
}
