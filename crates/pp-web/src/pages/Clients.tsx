import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Checkbox,
  Chip,
  Input,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
  Select,
  SelectItem,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  Spinner,
} from "@heroui/react";
import { PageHeader, ConfirmDialog, StatusBadge, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getClients, createClient, updateClient, deleteClient, resetClientTraffic } from "../api/clients";
import type { CreateClientPayload } from "../api/clients";
import { getGroups } from "../api/groups";
import { Client, Group } from "../api/types";
import { formatBytes, formatDateTime } from "../utils/format";

const RESET_STRATEGIES = ["no_reset", "daily", "weekly", "monthly", "yearly"];
const CLIENT_STATUSES = ["active", "on_hold", "disabled", "expired"];

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
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [clients, setClients] = useState<Client[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(false);
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

  const fetch = async () => {
    setLoading(true);
    try {
      const [clientsRes, groupsRes] = await Promise.allSettled([
        getClients(page, perPage),
        getGroups(),
      ]);
      if (clientsRes.status === "fulfilled") {
        setClients(clientsRes.value.data);
        setTotal(clientsRes.value.pagination.total);
      }
      if (groupsRes.status === "fulfilled") {
        setGroups(groupsRes.value);
      }
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

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

  const handleCreate = async () => {
    try {
      const { status: _, ...payload } = buildPayload();
      await createClient(payload);
      setCreateOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleUpdate = async () => {
    if (!editClient) return;
    try {
      await updateClient(editClient.id, buildPayload());
      setEditClient(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteClientId) return;
    try {
      await deleteClient(deleteClientId);
      setDeleteClientId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleResetTraffic = async (client: Client) => {
    try {
      await resetClientTraffic(client.id);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const openEdit = (client: Client) => {
    resetForm(client);
    setEditClient(client);
  };

  const groupNames = (ids: string[]) => {
    return ids
      .map((id) => groups.find((g) => g.id === id)?.name || id)
      .join(", ") || "-";
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
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table removeWrapper aria-label="clients">
              <TableHeader>
                <TableColumn>{t("common.name")}</TableColumn>
                <TableColumn>{t("clients.email")}</TableColumn>
                <TableColumn>{t("common.status")}</TableColumn>
                <TableColumn>{t("clients.trafficUsed")} / {t("clients.trafficLimit")}</TableColumn>
                <TableColumn>{t("clients.allTimeUsed")}</TableColumn>
                <TableColumn>{t("clients.onHold")}</TableColumn>
                <TableColumn>{t("clients.expiryDate")}</TableColumn>
                <TableColumn>{t("clients.resetStrategy")}</TableColumn>
                <TableColumn>{t("clients.maxDevices")}</TableColumn>
                <TableColumn>{t("clients.groups")}</TableColumn>
                <TableColumn>{t("common.actions")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {clients.map((client) => (
                  <TableRow key={client.id}>
                    <TableCell>
                      <div className="flex items-center gap-2">
                        {client.name}
                        {client.is_exceeded && (
                          <Chip color="danger" size="sm" variant="flat">
                            {t("clients.exceeded")}
                          </Chip>
                        )}
                      </div>
                    </TableCell>
                    <TableCell>{client.email || "-"}</TableCell>
                    <TableCell>
                      <StatusBadge status={client.status} />
                    </TableCell>
                    <TableCell>
                      {formatBytes(client.traffic_used_bytes)} / {formatBytes(client.traffic_limit_bytes)}
                    </TableCell>
                    <TableCell>{formatBytes(client.all_time_used_bytes)}</TableCell>
                    <TableCell>
                      <div className="space-y-1">
                        {client.status === "on_hold" && (
                          <Chip color="warning" size="sm" variant="flat">
                            {t("clients.onHold")}
                          </Chip>
                        )}
                        {client.on_hold_timeout && (
                          <p className="text-xs text-muted-foreground">
                            {t("clients.onHoldTimeout")}: {formatDateTime(client.on_hold_timeout)}
                          </p>
                        )}
                        {client.on_hold_expire_duration_secs !== null && (
                          <p className="text-xs text-muted-foreground">
                            {t("clients.onHoldDuration")}: {formatDuration(client.on_hold_expire_duration_secs)}
                          </p>
                        )}
                      </div>
                    </TableCell>
                    <TableCell>{formatDateTime(client.expiry_date)}</TableCell>
                    <TableCell>{resetStrategyLabel(client.data_limit_reset_strategy)}</TableCell>
                    <TableCell>{client.max_devices ?? "-"}</TableCell>
                    <TableCell className="max-w-xs truncate">{groupNames(client.group_ids || [])}</TableCell>
                    <TableCell>
                      <div className="flex flex-wrap gap-2">
                        <Button size="sm" variant="flat" onPress={() => openEdit(client)}>
                          {t("common.edit")}
                        </Button>
                        <Button size="sm" variant="flat" onPress={() => handleResetTraffic(client)}>
                          {t("clients.resetTraffic")}
                        </Button>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteClientId(client.id)}
                        >
                          {t("common.delete")}
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardBody>
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

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("clients.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              isRequired
            />
            <Input
              label={t("clients.email")}
              value={form.email}
              onChange={(e) => setForm({ ...form, email: e.target.value })}
            />
            <Input
              type="number"
              label={t("clients.trafficLimit")}
              value={form.traffic_limit_bytes}
              onChange={(e) => setForm({ ...form, traffic_limit_bytes: e.target.value })}
            />
            <Input
              type="datetime-local"
              label={t("clients.expiryDate")}
              value={form.expiry_date}
              onChange={(e) => setForm({ ...form, expiry_date: e.target.value })}
            />
            <Input
              type="number"
              label={t("clients.resetDay")}
              value={form.reset_day}
              onChange={(e) => setForm({ ...form, reset_day: e.target.value })}
            />
            <Input
              type="number"
              label={t("clients.maxDevices")}
              value={form.max_devices}
              onChange={(e) => setForm({ ...form, max_devices: e.target.value })}
            />
            <Select
              label={t("clients.resetStrategy")}
              selectedKeys={[form.data_limit_reset_strategy]}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, data_limit_reset_strategy: value });
              }}
            >
              {RESET_STRATEGIES.map((strategy) => (
                <SelectItem key={strategy}>{resetStrategyLabel(strategy)}</SelectItem>
              ))}
            </Select>
            <Input
              type="number"
              label={t("clients.onHoldDuration")}
              value={form.on_hold_expire_duration_secs}
              onChange={(e) => setForm({ ...form, on_hold_expire_duration_secs: e.target.value })}
            />
            <Input
              type="datetime-local"
              label={t("clients.onHoldTimeout")}
              value={form.on_hold_timeout}
              onChange={(e) => setForm({ ...form, on_hold_timeout: e.target.value })}
            />
            <div className="space-y-2">
              <p className="text-sm font-medium">{t("clients.groups")}</p>
              <div className="flex flex-wrap gap-2">
                {groups.map((g) => (
                  <Checkbox
                    key={g.id}
                    isSelected={form.selectedGroups.has(g.id)}
                    onValueChange={(selected) => {
                      const next = new Set(form.selectedGroups);
                      if (selected) next.add(g.id);
                      else next.delete(g.id);
                      setForm({ ...form, selectedGroups: next });
                    }}
                  >
                    {g.name}
                  </Checkbox>
                ))}
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button color="primary" onPress={handleCreate}>
              {t("common.create")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      <Modal isOpen={!!editClient} onClose={() => setEditClient(null)}>
        <ModalContent>
          <ModalHeader>{t("clients.editTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
            <Input
              label={t("clients.email")}
              value={form.email}
              onChange={(e) => setForm({ ...form, email: e.target.value })}
            />
            <Select
              label={t("common.status")}
              selectedKeys={[form.status]}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, status: value });
              }}
            >
              {CLIENT_STATUSES.map((status) => (
                <SelectItem key={status}>{status}</SelectItem>
              ))}
            </Select>
            <Input
              type="number"
              label={t("clients.trafficLimit")}
              value={form.traffic_limit_bytes}
              onChange={(e) => setForm({ ...form, traffic_limit_bytes: e.target.value })}
            />
            <Input
              type="datetime-local"
              label={t("clients.expiryDate")}
              value={form.expiry_date}
              onChange={(e) => setForm({ ...form, expiry_date: e.target.value })}
            />
            <Input
              type="number"
              label={t("clients.resetDay")}
              value={form.reset_day}
              onChange={(e) => setForm({ ...form, reset_day: e.target.value })}
            />
            <Input
              type="number"
              label={t("clients.maxDevices")}
              value={form.max_devices}
              onChange={(e) => setForm({ ...form, max_devices: e.target.value })}
            />
            <Select
              label={t("clients.resetStrategy")}
              selectedKeys={[form.data_limit_reset_strategy]}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, data_limit_reset_strategy: value });
              }}
            >
              {RESET_STRATEGIES.map((strategy) => (
                <SelectItem key={strategy}>{resetStrategyLabel(strategy)}</SelectItem>
              ))}
            </Select>
            <Input
              type="number"
              label={t("clients.onHoldDuration")}
              value={form.on_hold_expire_duration_secs}
              onChange={(e) => setForm({ ...form, on_hold_expire_duration_secs: e.target.value })}
            />
            <Input
              type="datetime-local"
              label={t("clients.onHoldTimeout")}
              value={form.on_hold_timeout}
              onChange={(e) => setForm({ ...form, on_hold_timeout: e.target.value })}
            />
            <div className="space-y-2">
              <p className="text-sm font-medium">{t("clients.groups")}</p>
              <div className="flex flex-wrap gap-2">
                {groups.map((g) => (
                  <Checkbox
                    key={g.id}
                    isSelected={form.selectedGroups.has(g.id)}
                    onValueChange={(selected) => {
                      const next = new Set(form.selectedGroups);
                      if (selected) next.add(g.id);
                      else next.delete(g.id);
                      setForm({ ...form, selectedGroups: next });
                    }}
                  >
                    {g.name}
                  </Checkbox>
                ))}
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setEditClient(null)}>
              {t("common.cancel")}
            </Button>
            <Button color="primary" onPress={handleUpdate}>
              {t("common.update")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}
