import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import { PageHeader, Pagination, FormSelect, FormCheckbox, SearchInput } from "../components/ui";
import { usePagination, useDebouncedValue } from "../hooks/useCommon";
import { getOnlines } from "../api/onlines";
import { getNodes } from "../api/nodes";
import { getClients, getClientIps } from "../api/clients";
import { Client, Node, OnlineSession } from "../api/types";
import { formatDateTime, formatDurationSince } from "../utils/format";

export function Onlines() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination(1, 20);
  const [sessions, setSessions] = useState<OnlineSession[]>([]);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [nodeId, setNodeId] = useState("");
  const [clientId, setClientId] = useState("");
  const [search, setSearch] = useState("");
  const [autoRefresh, setAutoRefresh] = useState(false);
  const [loading, setLoading] = useState(false);
  const [ipsModal, setIpsModal] = useState<{ clientName: string; ips: string[] } | null>(null);
  const [ipsLoading, setIpsLoading] = useState(false);

  const debouncedSearch = useDebouncedValue(search);

  const nodeNames = useMemo(() => new Map(nodes.map((n) => [n.id, n.name])), [nodes]);
  const clientNames = useMemo(() => new Map(clients.map((c) => [c.id, c.name])), [clients]);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getOnlines(nodeId || undefined, clientId || undefined);
      setSessions(res.data);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
    setPage(1);
  }, [nodeId, clientId]);

  useEffect(() => {
    Promise.allSettled([getNodes(), getClients(1, 1000)]).then(([nodesRes, clientsRes]) => {
      if (nodesRes.status === "fulfilled") setNodes(nodesRes.value);
      if (clientsRes.status === "fulfilled") setClients(clientsRes.value.data);
    });
  }, []);

  useEffect(() => {
    if (!autoRefresh) return;
    const id = setInterval(fetch, 30000);
    return () => clearInterval(id);
  }, [autoRefresh, nodeId, clientId]);

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

  useEffect(() => {
    setTotal(filtered.length);
  }, [filtered.length, setTotal]);

  const display = filtered.slice((page - 1) * perPage, page * perPage);

  const openIps = async (session: OnlineSession) => {
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
          onChange={setNodeId}
          options={nodeOptions}
          className="min-w-[200px]"
        />
        <FormSelect
          label={t("onlines.client")}
          value={clientId}
          onChange={setClientId}
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
              <Table aria-label="onlines">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("onlines.client")}</Table.Column>
                      <Table.Column>{t("onlines.node")}</Table.Column>
                      <Table.Column>{t("onlines.ip")}</Table.Column>
                      <Table.Column>{t("onlines.inbound")}</Table.Column>
                      <Table.Column>{t("onlines.connectedAt")}</Table.Column>
                      <Table.Column>{t("onlines.duration")}</Table.Column>
                      <Table.Column>{t("onlines.lastActive")}</Table.Column>
                      <Table.Column>{t("common.actions")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("onlines.empty")}
                        </div>
                      )}
                    >
                      {display.map((s) => (
                        <Table.Row key={s.id}>
                          <Table.Cell>{clientNames.get(s.client_id) || s.client_id}</Table.Cell>
                          <Table.Cell>{nodeNames.get(s.node_id) || s.node_id}</Table.Cell>
                          <Table.Cell>{s.ip_address}</Table.Cell>
                          <Table.Cell>{s.inbound_tag || "-"}</Table.Cell>
                          <Table.Cell>{formatDateTime(s.connected_at)}</Table.Cell>
                          <Table.Cell>{formatDurationSince(s.connected_at)}</Table.Cell>
                          <Table.Cell>{formatDateTime(s.last_active_at)}</Table.Cell>
                          <Table.Cell>
                            <Button size="sm" variant="ghost" onPress={() => openIps(s)}>
                              {t("onlines.historyIps")}
                            </Button>
                          </Table.Cell>
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

      <Modal.Backdrop
        isOpen={!!ipsModal}
        onOpenChange={(open) => {
          if (!open) setIpsModal(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {t("onlines.historyIpsTitle", { name: ipsModal?.clientName ?? "" })}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body>
              {ipsLoading ? (
                <div className="flex h-24 items-center justify-center">
                  <Spinner />
                </div>
              ) : ipsModal && ipsModal.ips.length > 0 ? (
                <ul className="space-y-1">
                  {ipsModal.ips.map((ip) => (
                    <li key={ip} className="rounded-md bg-surface-secondary px-3 py-1.5 text-sm">
                      {ip}
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="text-sm text-muted-foreground">{t("onlines.noIps")}</p>
              )}
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setIpsModal(null)}>
                {t("common.close")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
