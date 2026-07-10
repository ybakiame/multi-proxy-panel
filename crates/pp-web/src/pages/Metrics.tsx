import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  Pagination,
  FormSelect,
  FormCheckbox,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getMetrics } from "../api/metrics";
import { getNodes } from "../api/nodes";
import { Metric, Node } from "../api/types";
import { formatBytes, formatDateTime } from "../utils/format";

export function Metrics() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } =
    usePagination(1, 20);
  const [metrics, setMetrics] = useState<Metric[]>([]);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [nodeId, setNodeId] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [loading, setLoading] = useState(false);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getMetrics(nodeId || undefined);
      setMetrics(res.data);
      setTotal(res.data.length);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [nodeId, page, perPage]);

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(fetch, 30000);
    return () => clearInterval(id);
  }, [autoRefresh, nodeId]);

  useEffect(() => {
    getNodes()
      .then(setNodes)
      .catch(() => {});
  }, []);

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
        <Button onPress={fetch}>{t("common.refresh")}</Button>
      </div>
      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table>
                <Table.ScrollContainer>
                  <Table.Content aria-label="metrics">
                    <Table.Header>
                      <Table.Column isRowHeader>{t("nodes.name")}</Table.Column>
                      <Table.Column>{t("metrics.cpu")}</Table.Column>
                      <Table.Column>{t("metrics.memory")}</Table.Column>
                      <Table.Column>{t("metrics.disk")}</Table.Column>
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
                            <Table.Cell>
                              {node ? node.name : m.node_id}
                            </Table.Cell>
                            <Table.Cell>{m.cpu_percent.toFixed(2)}%</Table.Cell>
                            <Table.Cell>
                              {formatBytes(m.mem_used)} /{" "}
                              {formatBytes(m.mem_total)}
                            </Table.Cell>
                            <Table.Cell>
                              {formatBytes(m.disk_used)} /{" "}
                              {formatBytes(m.disk_total)}
                            </Table.Cell>
                            <Table.Cell>
                              {m.load_avg1.toFixed(2)} /{" "}
                              {m.load_avg5.toFixed(2)} /{" "}
                              {m.load_avg15.toFixed(2)}
                            </Table.Cell>
                            <Table.Cell>
                              {formatDateTime(m.timestamp)}
                            </Table.Cell>
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
