import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Checkbox,
  Select,
  SelectItem,
  Spinner,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
} from "@heroui/react";
import { PageHeader, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getMetrics } from "../api/metrics";
import { getNodes } from "../api/nodes";
import { Metric, Node } from "../api/types";
import { formatBytes, formatDateTime } from "../utils/format";

export function Metrics() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination(1, 20);
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
    getNodes().then(setNodes).catch(() => {});
  }, []);

  const display = metrics.slice((page - 1) * perPage, page * perPage);

  return (
    <div className="space-y-4">
      <PageHeader title={t("metrics.title")} />
      <div className="flex flex-wrap items-end gap-4">
        <Select
          label={t("metrics.filterByNode")}
          items={[
            { id: "", name: t("common.all") },
            ...nodes.map((node) => ({ id: node.id, name: node.name })),
          ]}
          selectedKeys={nodeId ? [nodeId] : []}
          onSelectionChange={(keys) => {
            const value = (Array.from(keys)[0] as string) || "";
            setNodeId(value);
            setPage(1);
          }}
          className="min-w-[240px]"
        >
          {(item) => <SelectItem key={item.id}>{item.name}</SelectItem>}
        </Select>
        <Checkbox isSelected={autoRefresh} onValueChange={setAutoRefresh}>
          {t("metrics.autoRefresh")}
        </Checkbox>
        <Button onPress={fetch}>{t("common.refresh")}</Button>
      </div>
      <Card>
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table removeWrapper aria-label="metrics">
                <TableHeader>
                  <TableColumn>{t("nodes.name")}</TableColumn>
                  <TableColumn>{t("metrics.cpu")}</TableColumn>
                  <TableColumn>{t("metrics.memory")}</TableColumn>
                  <TableColumn>{t("metrics.disk")}</TableColumn>
                  <TableColumn>{t("metrics.loadAvg")}</TableColumn>
                  <TableColumn>{t("metrics.timestamp")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("metrics.empty")}>
                  {display.map((m) => {
                    const node = nodes.find((n) => n.id === m.node_id);
                    return (
                      <TableRow key={m.id}>
                        <TableCell>{node ? node.name : m.node_id}</TableCell>
                        <TableCell>{m.cpu_percent.toFixed(2)}%</TableCell>
                        <TableCell>
                          {formatBytes(m.mem_used)} / {formatBytes(m.mem_total)}
                        </TableCell>
                        <TableCell>
                          {formatBytes(m.disk_used)} / {formatBytes(m.disk_total)}
                        </TableCell>
                        <TableCell>
                          {m.load_avg1.toFixed(2)} / {m.load_avg5.toFixed(2)} /{" "}
                          {m.load_avg15.toFixed(2)}
                        </TableCell>
                        <TableCell>{formatDateTime(m.timestamp)}</TableCell>
                      </TableRow>
                    );
                  })}
                </TableBody>
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
        </CardBody>
      </Card>
    </div>
  );
}
