import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { PageHeader, Pagination, FormSelect, FormCheckbox, SearchInput } from "../components/ui";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { Spinner } from "../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { usePagination, useDebouncedValue } from "../hooks/useCommon";
import { getOnlines } from "../api/onlines";
import { getNodes } from "../api/nodes";
import { getClients, getClientIps } from "../api/clients";
import { formatDateTime, formatDurationSince } from "../utils/format";

const onlinesQueryKey = "onlines";
const nodesQueryKey = "nodes";
const clientsQueryKey = "clients";

export function Onlines() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination(1, 20);
  const [nodeId, setNodeId] = useState("");
  const [clientId, setClientId] = useState("");
  const [search, setSearch] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [ipsModal, setIpsModal] = useState<{ clientName: string; ips: string[] } | null>(null);
  const [ipsLoading, setIpsLoading] = useState(false);

  const debouncedSearch = useDebouncedValue(search);

  const { data: sessionsData, isLoading } = useQuery({
    queryKey: [onlinesQueryKey, { nodeId: nodeId || undefined, clientId: clientId || undefined }],
    queryFn: () => getOnlines(nodeId || undefined, clientId || undefined),
    refetchInterval: autoRefresh ? 30000 : false,
  });

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const { data: clientsData } = useQuery({
    queryKey: [clientsQueryKey, { page: 1, perPage: 1000 }],
    queryFn: () => getClients(1, 1000),
  });

  const sessions = sessionsData?.data ?? [];
  const clients = clientsData?.data ?? [];

  const nodeNames = useMemo(() => new Map(nodes.map((n) => [n.id, n.name])), [nodes]);
  const clientNames = useMemo(() => new Map(clients.map((c) => [c.id, c.name])), [clients]);

  const filtered = useMemo(() => {
    const keyword = debouncedSearch.trim().toLowerCase();
    if (!keyword) return sessions;
    return sessions.filter(
      (s) =>
        s.ip_address.toLowerCase().includes(keyword) ||
        (clientNames.get(s.client_id) || "").toLowerCase().includes(keyword) ||
        (nodeNames.get(s.node_id) || "").toLowerCase().includes(keyword),
    );
  }, [sessions, debouncedSearch, clientNames, nodeNames]);

  const total = filtered.length;
  const totalPages = Math.max(1, Math.ceil(total / perPage));
  const display = filtered.slice((page - 1) * perPage, page * perPage);

  const handleNodeChange = (value: string) => {
    setNodeId(value);
    setPage(1);
  };

  const handleClientChange = (value: string) => {
    setClientId(value);
    setPage(1);
  };

  const handleRefresh = () => {
    queryClient.invalidateQueries({ queryKey: [onlinesQueryKey] });
  };

  const openIps = async (session: { client_id: string; ip_address: string }) => {
    const clientName = clientNames.get(session.client_id) || session.client_id;
    setIpsLoading(true);
    setIpsModal({ clientName, ips: [] });
    try {
      const res = await getClientIps(session.client_id);
      setIpsModal({ clientName, ips: res.ips });
    } catch {
      setIpsModal(null);
    } finally {
      setIpsLoading(false);
    }
  };

  const nodeOptions = [
    { id: "", label: t("common.all") },
    ...nodes.map((node) => ({ id: node.id, label: node.name })),
  ];
  const clientOptions = [
    { id: "", label: t("common.all") },
    ...clients.map((client) => ({ id: client.id, label: client.name })),
  ];

  return (
    <div className="space-y-4">
      <PageHeader title={t("onlines.title")} />
      <div className="flex flex-wrap items-end gap-4">
        <FormSelect
          label={t("onlines.node")}
          value={nodeId}
          onChange={handleNodeChange}
          options={nodeOptions}
          className="min-w-[200px]"
        />
        <FormSelect
          label={t("onlines.client")}
          value={clientId}
          onChange={handleClientChange}
          options={clientOptions}
          className="min-w-[200px]"
        />
        <SearchInput
          value={search}
          onChange={(value) => {
            setSearch(value);
            setPage(1);
          }}
          placeholder={t("onlines.searchPlaceholder")}
        />
        <FormCheckbox isSelected={autoRefresh} onChange={setAutoRefresh}>
          {t("metrics.autoRefresh")}
        </FormCheckbox>
        <Button onClick={handleRefresh}>{t("common.refresh")}</Button>
      </div>
      <Card>
        <CardContent>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="onlines">
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("onlines.client")}</TableHead>
                    <TableHead>{t("onlines.node")}</TableHead>
                    <TableHead>{t("onlines.ip")}</TableHead>
                    <TableHead>{t("onlines.inbound")}</TableHead>
                    <TableHead>{t("onlines.connectedAt")}</TableHead>
                    <TableHead>{t("onlines.duration")}</TableHead>
                    <TableHead>{t("onlines.lastActive")}</TableHead>
                    <TableHead>{t("common.actions")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {display.length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={8} className="p-4 text-center text-muted-foreground">
                        {t("onlines.empty")}
                      </TableCell>
                    </TableRow>
                  ) : (
                    display.map((s) => (
                      <TableRow key={s.id}>
                        <TableCell>{clientNames.get(s.client_id) || s.client_id}</TableCell>
                        <TableCell>{nodeNames.get(s.node_id) || s.node_id}</TableCell>
                        <TableCell>{s.ip_address}</TableCell>
                        <TableCell>{s.inbound_tag || "-"}</TableCell>
                        <TableCell>{formatDateTime(s.connected_at)}</TableCell>
                        <TableCell>{formatDurationSince(s.connected_at)}</TableCell>
                        <TableCell>{formatDateTime(s.last_active_at)}</TableCell>
                        <TableCell>
                          <Button size="sm" variant="ghost" onClick={() => openIps(s)}>
                            {t("onlines.historyIps")}
                          </Button>
                        </TableCell>
                      </TableRow>
                    ))
                  )}
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
        </CardContent>
      </Card>

      <Dialog
        open={!!ipsModal}
        onOpenChange={(open) => {
          if (!open) setIpsModal(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>
              {t("onlines.historyIpsTitle", { name: ipsModal?.clientName ?? "" })}
            </DialogTitle>
          </DialogHeader>
          <div>
            {ipsLoading ? (
              <div className="flex h-24 items-center justify-center">
                <Spinner />
              </div>
            ) : ipsModal && ipsModal.ips.length > 0 ? (
              <ul className="space-y-1">
                {ipsModal.ips.map((ip) => (
                  <li key={ip} className="rounded-md bg-muted px-3 py-1.5 text-sm">
                    {ip}
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-sm text-muted-foreground">{t("onlines.noIps")}</p>
            )}
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setIpsModal(null)}>
              {t("common.close")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
