import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { PageHeader, ConfirmDialog, Pagination, DataTable } from "../components/ui";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import { usePagination } from "../hooks/useCommon";
import {
  getProtocols,
  createProtocol,
  updateProtocol,
  deleteProtocol,
  generateRealityKeys,
} from "../api/protocols";
import { ProtocolConfig } from "../api/types";
import { ProtocolForm, ProtocolFormDialog } from "./ProtocolFormDialog";

const protocolsQueryKey = "protocols";

const defaultForm: ProtocolForm = {
  name: "",
  protocol_type: "vless_reality",
  core_type: "sing-box",
  listen_address: "0.0.0.0",
  listen_port: "",
  tls_enabled: true,
  uuid: "",
  password: "",
  flow: "xtls-rprx-vision",
  dest: "",
  server_names: "",
  private_key: "",
  public_key: "",
  short_id: "",
  path: "/xhttp",
  host: "",
  mode: "auto",
  obfs_type: "none",
  obfs_password: "",
  up_mbps: "",
  down_mbps: "",
  masquerade: "",
};

export function Protocols() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingProtocol, setEditingProtocol] = useState<ProtocolConfig | null>(null);
  const [deleteProtocolId, setDeleteProtocolId] = useState<string | null>(null);
  const [form, setForm] = useState<ProtocolForm>(defaultForm);

  const { data: protocolsData, isLoading } = useQuery({
    queryKey: [protocolsQueryKey, { page, perPage }],
    queryFn: () => getProtocols(page, perPage),
  });

  const protocols = protocolsData?.data ?? [];
  const total = protocolsData?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const createMutation = useMutation({
    mutationFn: (payload: {
      name: string;
      protocol_type: string;
      core_type: string;
      listen_address: string;
      listen_port: number;
      settings: Record<string, unknown>;
      tls_settings?: Record<string, unknown>;
    }) => createProtocol(payload),
    onSuccess: () => {
      setIsModalOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [protocolsQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: {
      id: string;
      data: {
        name: string;
        protocol_type: string;
        core_type: string;
        listen_address: string;
        listen_port: number;
        settings: Record<string, unknown>;
        tls_settings?: Record<string, unknown>;
      };
    }) => updateProtocol(payload.id, payload.data),
    onSuccess: () => {
      setIsModalOpen(false);
      setEditingProtocol(null);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [protocolsQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteProtocol,
    onSuccess: () => {
      setDeleteProtocolId(null);
      queryClient.invalidateQueries({ queryKey: [protocolsQueryKey] });
    },
  });

  const realityKeysMutation = useMutation({
    mutationFn: generateRealityKeys,
    onSuccess: (keys) => {
      setForm({
        ...form,
        private_key: keys.private_key || "",
        public_key: keys.public_key || "",
        short_id: keys.short_id || "",
      });
    },
  });

  const parseSettings = (settings: Record<string, unknown>): Partial<ProtocolForm> => {
    return {
      uuid: (settings.uuid as string) || "",
      password: (settings.password as string) || "",
      flow: (settings.flow as string) || "xtls-rprx-vision",
      dest: (settings.dest as string) || "",
      server_names: Array.isArray(settings.server_names)
        ? (settings.server_names as string[]).join(", ")
        : "",
      private_key: (settings.private_key as string) || "",
      public_key: (settings.public_key as string) || "",
      short_id: (settings.short_id as string) || "",
      path: (settings.path as string) || "",
      host: (settings.host as string) || "",
      mode: (settings.mode as string) || "auto",
      obfs_type: (settings.obfs_type as string) || "none",
      obfs_password: (settings.obfs_password as string) || "",
      up_mbps: settings.up_mbps !== undefined ? String(settings.up_mbps) : "",
      down_mbps: settings.down_mbps !== undefined ? String(settings.down_mbps) : "",
      masquerade: (settings.masquerade as string) || "",
    };
  };

  const resetForm = (protocol?: ProtocolConfig) => {
    if (protocol) {
      setForm({
        ...defaultForm,
        ...parseSettings(protocol.settings || {}),
        name: protocol.name,
        protocol_type: protocol.protocol_type,
        core_type: protocol.core_type,
        listen_address: protocol.listen_address,
        listen_port: protocol.listen_port.toString(),
        tls_enabled:
          protocol.protocol_type !== "vless_reality" ||
          (!!protocol.tls_settings && Object.keys(protocol.tls_settings).length > 0),
      });
    } else {
      setForm(defaultForm);
    }
  };

  const buildSettings = (): Record<string, unknown> => {
    switch (form.protocol_type) {
      case "vless_reality":
        return {
          uuid: form.uuid,
          flow: form.flow,
          dest: form.dest,
          server_names: form.server_names
            .split(",")
            .map((s) => s.trim())
            .filter(Boolean),
          private_key: form.private_key,
          public_key: form.public_key,
          short_id: form.short_id,
        };
      case "vless_xhttp":
        return {
          uuid: form.uuid,
          path: form.path,
          host: form.host,
          mode: form.mode,
        };
      case "hysteria2":
        return {
          password: form.password,
          obfs_type: form.obfs_type,
          obfs_password: form.obfs_password,
          up_mbps: Number(form.up_mbps) || 0,
          down_mbps: Number(form.down_mbps) || 0,
        };
      case "anytls":
        return {
          password: form.password,
          masquerade: form.masquerade,
        };
      default:
        return {};
    }
  };

  const buildTlsSettings = (): Record<string, unknown> | undefined => {
    if (!form.tls_enabled) return undefined;
    // Legacy detail fields (domain/certFile/cert_id) on the existing record
    // are preserved so old configs keep working until a binding override
    // takes over; new configs only carry the enabled flag.
    return { ...editingProtocol?.tls_settings, enabled: true };
  };

  const buildPayload = () => {
    return {
      name: form.name,
      protocol_type: form.protocol_type,
      core_type: form.core_type,
      listen_address: form.listen_address,
      listen_port: Number(form.listen_port) || 0,
      settings: buildSettings(),
      tls_settings: buildTlsSettings(),
    };
  };

  const handleCreate = () => {
    createMutation.mutate(buildPayload());
  };

  const handleUpdate = () => {
    if (!editingProtocol) return;
    updateMutation.mutate({ id: editingProtocol.id, data: buildPayload() });
  };

  const handleDelete = () => {
    if (!deleteProtocolId) return;
    deleteMutation.mutate(deleteProtocolId);
  };

  const handleGenerateRealityKeys = () => {
    realityKeysMutation.mutate();
  };

  const generateRandomPort = (): number => {
    const usedPorts = new Set(
      protocols.filter((p) => p.id !== editingProtocol?.id).map((p) => p.listen_port),
    );
    let port = 0;
    for (let attempt = 0; attempt < 100; attempt++) {
      port = 10000 + Math.floor(Math.random() * 50001);
      if (!usedPorts.has(port)) return port;
    }
    return port;
  };

  const openCreate = () => {
    resetForm();
    setEditingProtocol(null);
    setIsModalOpen(true);
  };

  const openEdit = (protocol: ProtocolConfig) => {
    resetForm(protocol);
    setEditingProtocol(protocol);
    setIsModalOpen(true);
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("protocols.title")}
        action={{
          label: t("protocols.create"),
          onClick: openCreate,
        }}
      />

      <Card>
        <CardContent>
          <DataTable
            aria-label="protocols"
            columns={7}
            loading={isLoading}
            empty={protocols.length === 0 ? t("common.empty") : undefined}
            header={
              <TableRow>
                <TableHead>{t("common.name")}</TableHead>
                <TableHead>{t("protocols.type")}</TableHead>
                <TableHead>{t("protocols.core")}</TableHead>
                <TableHead>Version</TableHead>
                <TableHead>{t("protocols.listen")}</TableHead>
                <TableHead>{t("protocols.port")}</TableHead>
                <TableHead>{t("common.actions")}</TableHead>
              </TableRow>
            }
          >
            {protocols.map((protocol) => (
              <TableRow key={protocol.id}>
                <TableCell>{protocol.name}</TableCell>
                <TableCell>{protocol.protocol_type}</TableCell>
                <TableCell>{protocol.core_type}</TableCell>
                <TableCell>{protocol.core_version || "-"}</TableCell>
                <TableCell>{protocol.listen_address}</TableCell>
                <TableCell>{protocol.listen_port}</TableCell>
                <TableCell>
                  <div className="flex gap-2">
                    <Button size="sm" variant="ghost" onClick={() => openEdit(protocol)}>
                      {t("common.edit")}
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      onClick={() => setDeleteProtocolId(protocol.id)}
                    >
                      {t("common.delete")}
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            ))}
          </DataTable>
        </CardContent>
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
        title={t("protocols.deleteTitle")}
        isOpen={!!deleteProtocolId}
        onClose={() => setDeleteProtocolId(null)}
        onConfirm={handleDelete}
      >
        {t("protocols.deleteConfirm")}
      </ConfirmDialog>

      <ProtocolFormDialog
        isOpen={isModalOpen}
        onOpenChange={setIsModalOpen}
        editingProtocol={editingProtocol}
        form={form}
        onFormChange={setForm}
        onSubmit={editingProtocol ? handleUpdate : handleCreate}
        onGenerateRealityKeys={handleGenerateRealityKeys}
        onGenerateRandomPort={() => setForm({ ...form, listen_port: String(generateRandomPort()) })}
      />
    </div>
  );
}
