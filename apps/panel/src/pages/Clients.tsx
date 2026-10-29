import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router-dom";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import {
  PageHeader,
  ConfirmDialog,
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
import { ClientsTable } from "./ClientsTable";
import { ClientSubscriptionDialog } from "./ClientSubscriptionDialog";

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

export function Clients() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [editClient, setEditClient] = useState<Client | null>(null);
  const [subscriptionClient, setSubscriptionClient] = useState<Client | null>(null);
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

      <ClientsTable
        clients={clients}
        groups={groups}
        isLoading={isLoading}
        resetStrategyLabel={resetStrategyLabel}
        onEdit={openEdit}
        onResetTraffic={handleResetTraffic}
        onTrafficDetail={(client) => navigate(`/traffic?client_id=${client.id}`)}
        onSubscription={setSubscriptionClient}
        onDelete={setDeleteClientId}
      />

      <ClientSubscriptionDialog
        key={subscriptionClient?.id}
        client={subscriptionClient}
        onClose={() => setSubscriptionClient(null)}
      />

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

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("clients.createTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleCreate}>{t("common.create")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog
        open={!!editClient}
        onOpenChange={(open) => {
          if (!open) setEditClient(null);
        }}
      >
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("clients.editTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setEditClient(null)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleUpdate}>{t("common.update")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
