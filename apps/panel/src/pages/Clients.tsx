import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Badge, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  StatusBadge,
  Pagination,
  FormInput,
  FormSelect,
  FormCheckbox,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getClients,
  createClient,
  updateClient,
  deleteClient,
  resetClientTraffic,
} from "../api/clients";
import type { CreateClientPayload } from "../api/clients";
import { getGroups } from "../api/groups";
import { Client } from "../api/types";
import { formatBytes, formatDateTime } from "../utils/format";

const RESET_STRATEGIES = ["no_reset", "daily", "weekly", "monthly", "yearly"];
const CLIENT_STATUSES = ["active", "on_hold", "disabled", "expired"];

const clientsQueryKey = "clients";
const groupsQueryKey = "groups";

interface ClientForm {
  name: string;
  email: string;
  traffic_limit_bytes: string;
  expiry_date: string;
  reset_day: string;
  max_devices: string;
  data_limit_reset_strategy: string;
  on_hold_expire_duration_secs: string;
  on_hold_timeout: string;
  status: string;
  selectedGroups: Set<string>;
}

function toDateTimeLocal(value: string | null): string {
  if (!value) return "";
  const d = new Date(value);
  if (isNaN(d.getTime())) return "";
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  const hours = String(d.getHours()).padStart(2, "0");
  const minutes = String(d.getMinutes()).padStart(2, "0");
  return `${year}-${month}-${day}T${hours}:${minutes}`;
}

function fromDateTimeLocal(value: string): string | undefined {
  if (!value) return undefined;
  const d = new Date(value);
  if (isNaN(d.getTime())) return undefined;
  return d.toISOString();
}

function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds === undefined) return "-";
  const parts: string[] = [];
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const mins = Math.floor((seconds % 3600) / 60);
  const secs = seconds % 60;
  if (days > 0) parts.push(`${days}d`);
  if (hours > 0) parts.push(`${hours}h`);
  if (mins > 0) parts.push(`${mins}m`);
  if (secs > 0) parts.push(`${secs}s`);
  return parts.join(" ") || "0s";
}

export function Clients() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [editClient, setEditClient] = useState<Client | null>(null);
  const [deleteClientId, setDeleteClientId] = useState<string | null>(null);
  const [form, setForm] = useState<ClientForm>({
    name: "",
    email: "",
    traffic_limit_bytes: "",
    expiry_date: "",
    reset_day: "",
    max_devices: "",
    data_limit_reset_strategy: "no_reset",
    on_hold_expire_duration_secs: "",
    on_hold_timeout: "",
    status: "active",
    selectedGroups: new Set<string>(),
  });

  const { data: clientsData, isLoading } = useQuery({
    queryKey: [clientsQueryKey, { page, perPage }],
    queryFn: () => getClients(page, perPage),
  });

  const clients = clientsData?.data ?? [];
  const total = clientsData?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const { data: groups = [] } = useQuery({
    queryKey: [groupsQueryKey],
    queryFn: getGroups,
  });

  const createMutation = useMutation({
    mutationFn: (payload: CreateClientPayload) => createClient(payload),
    onSuccess: () => {
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [clientsQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: { id: string; data: Partial<CreateClientPayload> }) =>
      updateClient(payload.id, payload.data),
    onSuccess: () => {
      setEditClient(null);
      queryClient.invalidateQueries({ queryKey: [clientsQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteClient,
    onSuccess: () => {
      setDeleteClientId(null);
      queryClient.invalidateQueries({ queryKey: [clientsQueryKey] });
    },
  });

  const resetTrafficMutation = useMutation({
    mutationFn: resetClientTraffic,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [clientsQueryKey] });
    },
  });

  const resetForm = (client?: Client) => {
    if (client) {
      setForm({
        name: client.name,
        email: client.email || "",
        traffic_limit_bytes: client.traffic_limit_bytes?.toString() || "",
        expiry_date: toDateTimeLocal(client.expiry_date),
        reset_day: client.reset_day?.toString() || "",
        max_devices: client.max_devices?.toString() || "",
        data_limit_reset_strategy: client.data_limit_reset_strategy || "no_reset",
        on_hold_expire_duration_secs: client.on_hold_expire_duration_secs?.toString() || "",
        on_hold_timeout: toDateTimeLocal(client.on_hold_timeout),
        status: client.status || "active",
        selectedGroups: new Set(client.group_ids || []),
      });
    } else {
      setForm({
        name: "",
        email: "",
        traffic_limit_bytes: "",
        expiry_date: "",
        reset_day: "",
        max_devices: "",
        data_limit_reset_strategy: "no_reset",
        on_hold_expire_duration_secs: "",
        on_hold_timeout: "",
        status: "active",
        selectedGroups: new Set<string>(),
      });
    }
  };

  const buildPayload = (): CreateClientPayload => {
    return {
      name: form.name,
      email: form.email || undefined,
      traffic_limit_bytes: form.traffic_limit_bytes ? Number(form.traffic_limit_bytes) : undefined,
      expiry_date: fromDateTimeLocal(form.expiry_date),
      reset_day: form.reset_day ? Number(form.reset_day) : undefined,
      max_devices: form.max_devices ? Number(form.max_devices) : undefined,
      data_limit_reset_strategy: form.data_limit_reset_strategy,
      on_hold_expire_duration_secs: form.on_hold_expire_duration_secs
        ? Number(form.on_hold_expire_duration_secs)
        : undefined,
      on_hold_timeout: fromDateTimeLocal(form.on_hold_timeout),
      status: form.status,
      group_ids: Array.from(form.selectedGroups),
    };
  };

  const handleCreate = () => {
    const { status: _, ...payload } = buildPayload();
    createMutation.mutate(payload);
  };

  const handleUpdate = () => {
    if (!editClient) return;
    updateMutation.mutate({ id: editClient.id, data: buildPayload() });
  };

  const handleDelete = () => {
    if (!deleteClientId) return;
    deleteMutation.mutate(deleteClientId);
  };

  const handleResetTraffic = (client: Client) => {
    resetTrafficMutation.mutate(client.id);
  };

  const openEdit = (client: Client) => {
    resetForm(client);
    setEditClient(client);
  };

  const groupNames = (ids: string[]) => {
    return ids.map((id) => groups.find((g) => g.id === id)?.name || id).join(", ") || "-";
  };

  const resetStrategyLabel = (strategy: string) => {
    const key = strategy.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
    return t(`clients.strategy.${key}` as any) || strategy;
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("clients.title")}
        action={{
          label: t("clients.create"),
          onClick: () => {
            resetForm();
            setCreateOpen(true);
          },
        }}
      />

      <Card>
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="clients">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("common.name")}</Table.Column>
                    <Table.Column>{t("clients.email")}</Table.Column>
                    <Table.Column>{t("common.status")}</Table.Column>
                    <Table.Column>
                      {t("clients.trafficUsed")} / {t("clients.trafficLimit")}
                    </Table.Column>
                    <Table.Column>{t("clients.allTimeUsed")}</Table.Column>
                    <Table.Column>{t("clients.onHold")}</Table.Column>
                    <Table.Column>{t("clients.expiryDate")}</Table.Column>
                    <Table.Column>{t("clients.resetStrategy")}</Table.Column>
                    <Table.Column>{t("clients.maxDevices")}</Table.Column>
                    <Table.Column>{t("clients.groups")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </div>
                    )}
                  >
                    {clients.map((client) => (
                      <Table.Row key={client.id}>
                        <Table.Cell>
                          <div className="flex items-center gap-2">
                            {client.name}
                            {client.is_exceeded && (
                              <Badge color="danger" size="sm" variant="soft">
                                {t("clients.exceeded")}
                              </Badge>
                            )}
                          </div>
                        </Table.Cell>
                        <Table.Cell>{client.email || "-"}</Table.Cell>
                        <Table.Cell>
                          <StatusBadge status={client.status} />
                        </Table.Cell>
                        <Table.Cell>
                          {formatBytes(client.traffic_used_bytes)} /{" "}
                          {formatBytes(client.traffic_limit_bytes)}
                        </Table.Cell>
                        <Table.Cell>{formatBytes(client.all_time_used_bytes)}</Table.Cell>
                        <Table.Cell>
                          <div className="space-y-1">
                            {client.status === "on_hold" && (
                              <Badge color="warning" size="sm" variant="soft">
                                {t("clients.onHold")}
                              </Badge>
                            )}
                            {client.on_hold_timeout && (
                              <p className="text-xs text-muted-foreground">
                                {t("clients.onHoldTimeout")}:{" "}
                                {formatDateTime(client.on_hold_timeout)}
                              </p>
                            )}
                            {client.on_hold_expire_duration_secs !== null && (
                              <p className="text-xs text-muted-foreground">
                                {t("clients.onHoldDuration")}:{" "}
                                {formatDuration(client.on_hold_expire_duration_secs)}
                              </p>
                            )}
                          </div>
                        </Table.Cell>
                        <Table.Cell>{formatDateTime(client.expiry_date)}</Table.Cell>
                        <Table.Cell>
                          {resetStrategyLabel(client.data_limit_reset_strategy)}
                        </Table.Cell>
                        <Table.Cell>{client.max_devices ?? "-"}</Table.Cell>
                        <Table.Cell className="max-w-xs truncate">
                          {groupNames(client.group_ids || [])}
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex flex-wrap gap-2">
                            <Button size="sm" variant="ghost" onPress={() => openEdit(client)}>
                              {t("common.edit")}
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              onPress={() => handleResetTraffic(client)}
                            >
                              {t("clients.resetTraffic")}
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              onPress={() => navigate(`/traffic?client_id=${client.id}`)}
                            >
                              {t("clients.trafficDetail")}
                            </Button>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteClientId(client.id)}
                            >
                              {t("common.delete")}
                            </Button>
                          </div>
                        </Table.Cell>
                      </Table.Row>
                    ))}
                  </Table.Body>
                </Table.Content>
              </Table.ScrollContainer>
            </Table>
          )}
        </Card.Content>
      </Card>

      <Pagination
        page={page}
        totalPages={totalPages}
        perPage={perPage}
        total={total}
        onPageChange={setPage}
        onPerPageChange={setPerPage}
      />

      <ConfirmDialog
        title={t("clients.deleteTitle")}
        isOpen={!!deleteClientId}
        onClose={() => setDeleteClientId(null)}
        onConfirm={handleDelete}
      >
        {t("clients.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("clients.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormInput
                label={t("clients.email")}
                value={form.email}
                onChange={(value) => setForm({ ...form, email: value })}
              />
              <FormInput
                type="number"
                label={t("clients.trafficLimit")}
                value={form.traffic_limit_bytes}
                onChange={(value) => setForm({ ...form, traffic_limit_bytes: value })}
              />
              <FormInput
                type="datetime-local"
                label={t("clients.expiryDate")}
                value={form.expiry_date}
                onChange={(value) => setForm({ ...form, expiry_date: value })}
              />
              <FormInput
                type="number"
                label={t("clients.resetDay")}
                value={form.reset_day}
                onChange={(value) => setForm({ ...form, reset_day: value })}
              />
              <FormInput
                type="number"
                label={t("clients.maxDevices")}
                value={form.max_devices}
                onChange={(value) => setForm({ ...form, max_devices: value })}
              />
              <FormSelect
                label={t("clients.resetStrategy")}
                value={form.data_limit_reset_strategy}
                onChange={(value) => setForm({ ...form, data_limit_reset_strategy: value })}
                options={RESET_STRATEGIES.map((strategy) => ({
                  id: strategy,
                  label: resetStrategyLabel(strategy),
                }))}
              />
              <FormInput
                type="number"
                label={t("clients.onHoldDuration")}
                value={form.on_hold_expire_duration_secs}
                onChange={(value) => setForm({ ...form, on_hold_expire_duration_secs: value })}
              />
              <FormInput
                type="datetime-local"
                label={t("clients.onHoldTimeout")}
                value={form.on_hold_timeout}
                onChange={(value) => setForm({ ...form, on_hold_timeout: value })}
              />
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("clients.groups")}</p>
                <div className="flex flex-wrap gap-2">
                  {groups.map((g) => (
                    <FormCheckbox
                      key={g.id}
                      isSelected={form.selectedGroups.has(g.id)}
                      onChange={(selected) => {
                        const next = new Set(form.selectedGroups);
                        if (selected) next.add(g.id);
                        else next.delete(g.id);
                        setForm({ ...form, selectedGroups: next });
                      }}
                    >
                      {g.name}
                    </FormCheckbox>
                  ))}
                </div>
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setCreateOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleCreate}>{t("common.create")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <Modal.Backdrop
        isOpen={!!editClient}
        onOpenChange={(open) => {
          if (!open) setEditClient(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("clients.editTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
              />
              <FormInput
                label={t("clients.email")}
                value={form.email}
                onChange={(value) => setForm({ ...form, email: value })}
              />
              <FormSelect
                label={t("common.status")}
                value={form.status}
                onChange={(value) => setForm({ ...form, status: value })}
                options={CLIENT_STATUSES.map((status) => ({
                  id: status,
                  label: status,
                }))}
              />
              <FormInput
                type="number"
                label={t("clients.trafficLimit")}
                value={form.traffic_limit_bytes}
                onChange={(value) => setForm({ ...form, traffic_limit_bytes: value })}
              />
              <FormInput
                type="datetime-local"
                label={t("clients.expiryDate")}
                value={form.expiry_date}
                onChange={(value) => setForm({ ...form, expiry_date: value })}
              />
              <FormInput
                type="number"
                label={t("clients.resetDay")}
                value={form.reset_day}
                onChange={(value) => setForm({ ...form, reset_day: value })}
              />
              <FormInput
                type="number"
                label={t("clients.maxDevices")}
                value={form.max_devices}
                onChange={(value) => setForm({ ...form, max_devices: value })}
              />
              <FormSelect
                label={t("clients.resetStrategy")}
                value={form.data_limit_reset_strategy}
                onChange={(value) => setForm({ ...form, data_limit_reset_strategy: value })}
                options={RESET_STRATEGIES.map((strategy) => ({
                  id: strategy,
                  label: resetStrategyLabel(strategy),
                }))}
              />
              <FormInput
                type="number"
                label={t("clients.onHoldDuration")}
                value={form.on_hold_expire_duration_secs}
                onChange={(value) => setForm({ ...form, on_hold_expire_duration_secs: value })}
              />
              <FormInput
                type="datetime-local"
                label={t("clients.onHoldTimeout")}
                value={form.on_hold_timeout}
                onChange={(value) => setForm({ ...form, on_hold_timeout: value })}
              />
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("clients.groups")}</p>
                <div className="flex flex-wrap gap-2">
                  {groups.map((g) => (
                    <FormCheckbox
                      key={g.id}
                      isSelected={form.selectedGroups.has(g.id)}
                      onChange={(selected) => {
                        const next = new Set(form.selectedGroups);
                        if (selected) next.add(g.id);
                        else next.delete(g.id);
                        setForm({ ...form, selectedGroups: next });
                      }}
                    >
                      {g.name}
                    </FormCheckbox>
                  ))}
                </div>
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setEditClient(null)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleUpdate}>{t("common.update")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
