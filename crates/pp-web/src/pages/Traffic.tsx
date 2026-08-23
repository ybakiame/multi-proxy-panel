import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { Card, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination, FormSelect, FormInput, TrafficChart } from "../components/ui";
import type { TrafficPoint } from "../components/ui/TrafficChart";
import { usePagination } from "../hooks/useCommon";
import { getTraffic } from "../api/traffic";
import { getUsageSummary } from "../api/usage";
import { getNodes } from "../api/nodes";
import { getClients } from "../api/clients";
import { UsageSummaryItem } from "../api/types";
import { formatBytes, formatDateTime, toDateTimeLocal, fromDateTimeLocal } from "../utils/format";

interface SummaryState {
  today: number;
  week: number;
  topNode: UsageSummaryItem | null;
  topClient: UsageSummaryItem | null;
}

const trafficQueryKey = "traffic";
const nodesQueryKey = "nodes";
const clientsQueryKey = "clients";

export function Traffic() {
  const { t } = useTranslation();
  const [searchParams] = useSearchParams();
  const { page, perPage, setPage, setPerPage } = usePagination(1, 20);
  const [nodeId, setNodeId] = useState(searchParams.get("node_id") || "");
  const [clientId, setClientId] = useState(searchParams.get("client_id") || "");
  const [start, setStart] = useState(toDateTimeLocal(searchParams.get("start")));
  const [end, setEnd] = useState(toDateTimeLocal(searchParams.get("end")));

  const { data: trafficRes, isLoading: trafficLoading } = useQuery({
    queryKey: [trafficQueryKey, { nodeId, clientId, start, end }],
    queryFn: () =>
      getTraffic({
        nodeId: nodeId || undefined,
        clientId: clientId || undefined,
        start: fromDateTimeLocal(start),
        end: fromDateTimeLocal(end),
      }),
  });

  const records = trafficRes?.data ?? [];
  const total = records.length;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const { data: clientsData } = useQuery({
    queryKey: [clientsQueryKey, { page: 1, perPage: 1000 }],
    queryFn: () => getClients(1, 1000),
  });

  const clients = clientsData?.data ?? [];

  const { data: summary } = useQuery({
    queryKey: ["traffic-summary"],
    queryFn: async () => {
      const now = new Date();
      const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate());
      const startOfWeek = new Date(startOfToday.getTime() - 6 * 86400000);
      const sum = (items: { upload_bytes: number; download_bytes: number }[]) =>
        items.reduce((acc, r) => acc + r.upload_bytes + r.download_bytes, 0);

      const [todayRes, weekRes, topNodeRes, topClientRes] = await Promise.allSettled([
        getTraffic({ start: startOfToday.toISOString(), limit: 5000 }),
        getTraffic({ start: startOfWeek.toISOString(), limit: 5000 }),
        getUsageSummary("node", { limit: 1 }),
        getUsageSummary("client", { limit: 1 }),
      ]);

      return {
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
      };
    },
  });

  const summaryState: SummaryState = summary ?? {
    today: 0,
    week: 0,
    topNode: null,
    topClient: null,
  };

  const nodeNames = useMemo(() => new Map(nodes.map((n) => [n.id, n.name])), [nodes]);
  const clientNames = useMemo(() => new Map(clients.map((c) => [c.id, c.name])), [clients]);

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
    { label: t("traffic.todayTotal"), value: formatBytes(summaryState.today) },
    { label: t("traffic.weekTotal"), value: formatBytes(summaryState.week) },
    {
      label: t("traffic.topNode"),
      value: summaryState.topNode
        ? `${nodeNames.get(summaryState.topNode.id) || summaryState.topNode.id} (${formatBytes(summaryState.topNode.total_bytes)})`
        : "-",
    },
    {
      label: t("traffic.topClient"),
      value: summaryState.topClient
        ? `${clientNames.get(summaryState.topClient.id) || summaryState.topClient.id} (${formatBytes(summaryState.topClient.total_bytes)})`
        : "-",
    },
  ];

  const handleNodeChange = (value: string) => {
    setNodeId(value);
    setPage(1);
  };

  const handleClientChange = (value: string) => {
    setClientId(value);
    setPage(1);
  };

  const handleStartChange = (value: string) => {
    setStart(value);
    setPage(1);
  };

  const handleEndChange = (value: string) => {
    setEnd(value);
    setPage(1);
  };

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
          onChange={handleNodeChange}
          options={nodeOptions}
          className="min-w-[200px]"
        />
        <FormSelect
          label={t("traffic.client")}
          value={clientId}
          onChange={handleClientChange}
          options={clientOptions}
          className="min-w-[200px]"
        />
        <FormInput
          type="datetime-local"
          label={t("traffic.start")}
          value={start}
          onChange={handleStartChange}
        />
        <FormInput
          type="datetime-local"
          label={t("traffic.end")}
          value={end}
          onChange={handleEndChange}
        />
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
          {trafficLoading ? (
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
