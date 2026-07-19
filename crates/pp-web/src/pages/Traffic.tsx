import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { Card, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination, FormSelect, FormInput, TrafficChart } from "../components/ui";
import type { TrafficPoint } from "../components/ui/TrafficChart";
import { usePagination } from "../hooks/useCommon";
import { getTraffic } from "../api/traffic";
import { getUsageSummary } from "../api/usage";
import { getNodes } from "../api/nodes";
import { getClients } from "../api/clients";
import { Client, Node, TrafficRecord, UsageSummaryItem } from "../api/types";
import { formatBytes, formatDateTime, toDateTimeLocal, fromDateTimeLocal } from "../utils/format";

interface SummaryState {
  today: number;
  week: number;
  topNode: UsageSummaryItem | null;
  topClient: UsageSummaryItem | null;
}

export function Traffic() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination(1, 20);
  const [records, setRecords] = useState<TrafficRecord[]>([]);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [nodeId, setNodeId] = useState(searchParams.get("node_id") || "");
  const [clientId, setClientId] = useState(searchParams.get("client_id") || "");
  const [start, setStart] = useState(toDateTimeLocal(searchParams.get("start")));
  const [end, setEnd] = useState(toDateTimeLocal(searchParams.get("end")));
  const [summary, setSummary] = useState<SummaryState>({
    today: 0,
    week: 0,
    topNode: null,
    topClient: null,
  });
  const [loading, setLoading] = useState(false);

  const nodeNames = useMemo(() => new Map(nodes.map((n) => [n.id, n.name])), [nodes]);
  const clientNames = useMemo(() => new Map(clients.map((c) => [c.id, c.name])), [clients]);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getTraffic({
        nodeId: nodeId || undefined,
        clientId: clientId || undefined,
        start: fromDateTimeLocal(start),
        end: fromDateTimeLocal(end),
      });
      setRecords(res.data);
      setTotal(res.data.length);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
    setPage(1);
  }, [nodeId, clientId, start, end]);

  useEffect(() => {
    Promise.allSettled([getNodes(), getClients(1, 1000)]).then(([nodesRes, clientsRes]) => {
      if (nodesRes.status === "fulfilled") setNodes(nodesRes.value);
      if (clientsRes.status === "fulfilled") setClients(clientsRes.value.data);
    });
  }, []);

  useEffect(() => {
    const now = new Date();
    const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const startOfWeek = new Date(startOfToday.getTime() - 6 * 86400000);
    const sum = (items: { upload_bytes: number; download_bytes: number }[]) =>
      items.reduce((acc, r) => acc + r.upload_bytes + r.download_bytes, 0);

    Promise.allSettled([
      getTraffic({ start: startOfToday.toISOString(), limit: 5000 }),
      getTraffic({ start: startOfWeek.toISOString(), limit: 5000 }),
      getUsageSummary("node", { limit: 1 }),
      getUsageSummary("client", { limit: 1 }),
    ]).then(([todayRes, weekRes, topNodeRes, topClientRes]) => {
      setSummary({
        today: todayRes.status === "fulfilled" ? sum(todayRes.value.data) : 0,
        week: weekRes.status === "fulfilled" ? sum(weekRes.value.data) : 0,
        topNode:
          topNodeRes.status === "fulfilled" && topNodeRes.value.length > 0
            ? topNodeRes.value[0]
            : null,
        topClient:
          topClientRes.status === "fulfilled" && topClientRes.value.length > 0
            ? topClientRes.value[0]
            : null,
      });
    });
  }, []);

  const chartData = useMemo<TrafficPoint[]>(() => {
    const byHour = new Map<string, TrafficPoint>();
    for (const r of records) {
      const key = r.hour_bucket;
      const point = byHour.get(key) || {
        time: new Date(key).toLocaleString(undefined, {
          month: "numeric",
          day: "numeric",
          hour: "2-digit",
        }),
        upload: 0,
        download: 0,
      };
      point.upload += r.upload_bytes;
      point.download += r.download_bytes;
      byHour.set(key, point);
    }
    return Array.from(byHour.entries())
      .sort(([a], [b]) => new Date(a).getTime() - new Date(b).getTime())
      .map(([, point]) => point);
  }, [records]);

  const display = records.slice((page - 1) * perPage, page * perPage);

  const nodeOptions = [
    { id: "", label: t("common.all") },
    ...nodes.map((node) => ({ id: node.id, label: node.name })),
  ];
  const clientOptions = [
    { id: "", label: t("common.all") },
    ...clients.map((client) => ({ id: client.id, label: client.name })),
  ];

  const summaryCards = [
    { label: t("traffic.todayTotal"), value: formatBytes(summary.today) },
    { label: t("traffic.weekTotal"), value: formatBytes(summary.week) },
    {
      label: t("traffic.topNode"),
      value: summary.topNode
        ? `${nodeNames.get(summary.topNode.id) || summary.topNode.id} (${formatBytes(summary.topNode.total_bytes)})`
        : "-",
    },
    {
      label: t("traffic.topClient"),
      value: summary.topClient
        ? `${clientNames.get(summary.topClient.id) || summary.topClient.id} (${formatBytes(summary.topClient.total_bytes)})`
        : "-",
    },
  ];

  return (
    <div className="space-y-4">
      <PageHeader title={t("traffic.title")} />

      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {summaryCards.map((card) => (
          <Card key={card.label}>
            <Card.Content className="p-5">
              <p className="text-sm text-muted-foreground">{card.label}</p>
              <p className="mt-1 truncate text-xl font-bold" title={card.value}>
                {card.value}
              </p>
            </Card.Content>
          </Card>
        ))}
      </div>

      <div className="flex flex-wrap items-end gap-4">
        <FormSelect
          label={t("traffic.node")}
          value={nodeId}
          onChange={setNodeId}
          options={nodeOptions}
          className="min-w-[200px]"
        />
        <FormSelect
          label={t("traffic.client")}
          value={clientId}
          onChange={setClientId}
          options={clientOptions}
          className="min-w-[200px]"
        />
        <FormInput
          type="datetime-local"
          label={t("traffic.start")}
          value={start}
          onChange={setStart}
        />
        <FormInput type="datetime-local" label={t("traffic.end")} value={end} onChange={setEnd} />
      </div>

      <Card>
        <Card.Header>
          <h3 className="text-lg font-semibold">{t("traffic.chartTitle")}</h3>
        </Card.Header>
        <Card.Content>
          <TrafficChart data={chartData} />
        </Card.Content>
      </Card>

      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="traffic">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("traffic.hour")}</Table.Column>
                      <Table.Column>{t("traffic.node")}</Table.Column>
                      <Table.Column>{t("traffic.client")}</Table.Column>
                      <Table.Column>{t("traffic.upload")}</Table.Column>
                      <Table.Column>{t("traffic.download")}</Table.Column>
                      <Table.Column>{t("traffic.total")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("traffic.empty")}
                        </div>
                      )}
                    >
                      {display.map((r) => (
                        <Table.Row key={r.id}>
                          <Table.Cell>{formatDateTime(r.hour_bucket)}</Table.Cell>
                          <Table.Cell>
                            {r.node_id ? nodeNames.get(r.node_id) || r.node_id : "-"}
                          </Table.Cell>
                          <Table.Cell>
                            {r.client_id ? clientNames.get(r.client_id) || r.client_id : "-"}
                          </Table.Cell>
                          <Table.Cell>{formatBytes(r.upload_bytes)}</Table.Cell>
                          <Table.Cell>{formatBytes(r.download_bytes)}</Table.Cell>
                          <Table.Cell>{formatBytes(r.upload_bytes + r.download_bytes)}</Table.Cell>
                        </Table.Row>
                      ))}
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
