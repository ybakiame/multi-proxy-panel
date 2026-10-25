import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Card, CardContent } from "../components/ui/card";
import { Spinner } from "../components/ui/spinner";
import { PageHeader, ConfirmDialog, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getBindingsPaginated, createBinding, updateBinding, deleteBinding } from "../api/bindings";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { getCertificates } from "../api/certificates";
import { Binding, ManagedCertificate } from "../api/types";
import { BindingFormDialog, BindingForm, defaultForm } from "./BindingFormDialog";
import { BindingsTable } from "./BindingsTable";

const bindingsQueryKey = "bindings";
const nodesQueryKey = "nodes";
const protocolsQueryKey = "protocols";
const certificatesQueryKey = "certificates";

const parseOverrideJson = (value: string): Record<string, unknown> => {
  try {
    return JSON.parse(value) as Record<string, unknown>;
  } catch {
    return {};
  }
};

const tlsFieldsFromOverride = (
  override?: Record<string, unknown> | null,
): Pick<
  BindingForm,
  "tls_type" | "tls_cert_file" | "tls_key_file" | "tls_domain" | "tls_cert_id"
> => {
  const empty = {
    tls_type: "none" as const,
    tls_cert_file: "",
    tls_key_file: "",
    tls_domain: "",
    tls_cert_id: "",
  };
  const tls = override?.tls_settings as Record<string, unknown> | undefined;
  if (!tls) return empty;

  const certId = (tls.cert_id as string) || "";
  if (certId) return { ...empty, tls_type: "managed", tls_cert_id: certId };
  const domain = (tls.domain as string) || "";
  if (domain) return { ...empty, tls_type: "acme", tls_domain: domain };
  const certFile = (tls.certFile as string) || "";
  const keyFile = (tls.keyFile as string) || "";
  if (certFile || keyFile) {
    return { ...empty, tls_type: "certificate", tls_cert_file: certFile, tls_key_file: keyFile };
  }
  return empty;
};

export function Bindings() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [editBinding, setEditBinding] = useState<Binding | null>(null);
  const [deleteBindingId, setDeleteBindingId] = useState<string | null>(null);
  const [form, setForm] = useState<BindingForm>(defaultForm);

  const { data: bindingsData, isLoading } = useQuery({
    queryKey: [bindingsQueryKey, { page, perPage }],
    queryFn: () => getBindingsPaginated(page, perPage),
  });

  const bindings = bindingsData?.data ?? [];
  const total = bindingsData?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const { data: protocols = [] } = useQuery({
    queryKey: [protocolsQueryKey],
    queryFn: getAllProtocols,
  });

  const { data: certificates = [] } = useQuery<ManagedCertificate[]>({
    queryKey: [certificatesQueryKey],
    queryFn: () => getCertificates(),
  });

  const createMutation = useMutation({
    mutationFn: (payload: {
      node_id: string;
      protocol_config_id: string;
      is_active?: boolean;
      override_settings?: Record<string, unknown>;
    }) => createBinding(payload),
    onSuccess: () => {
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [bindingsQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: {
      id: string;
      data: { is_active?: boolean; override_settings?: Record<string, unknown> };
    }) => updateBinding(payload.id, payload.data),
    onSuccess: () => {
      setEditBinding(null);
      queryClient.invalidateQueries({ queryKey: [bindingsQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteBinding,
    onSuccess: () => {
      setDeleteBindingId(null);
      queryClient.invalidateQueries({ queryKey: [bindingsQueryKey] });
    },
  });

  const buildOverrideWithTls = (current: BindingForm): Record<string, unknown> => {
    const override = parseOverrideJson(current.override_settings);
    delete override.tls_settings;
    if (current.tls_type === "certificate") {
      if (current.tls_cert_file.trim() || current.tls_key_file.trim()) {
        override.tls_settings = {
          certFile: current.tls_cert_file.trim(),
          keyFile: current.tls_key_file.trim(),
        };
      }
    } else if (current.tls_type === "acme" && current.tls_domain.trim()) {
      override.tls_settings = { domain: current.tls_domain.trim() };
    } else if (current.tls_type === "managed" && current.tls_cert_id) {
      override.tls_settings = { cert_id: current.tls_cert_id };
    }
    return override;
  };

  const updateForm = (updates: Partial<BindingForm>) => {
    setForm((prev) => {
      const next = { ...prev, ...updates };
      if (updates.protocol_config_id !== undefined) {
        const nextProtocol = protocols.find((p) => p.id === updates.protocol_config_id);
        if (nextProtocol?.core_type !== "sing-box" && next.tls_type === "acme") {
          next.tls_type = "none";
          next.tls_domain = "";
        }
      }
      const override = buildOverrideWithTls(next);
      return { ...next, override_settings: JSON.stringify(override, null, 2) };
    });
  };

  const setOverrideSettings = (value: string) => {
    const override = parseOverrideJson(value);
    const tlsFields = tlsFieldsFromOverride(override);
    setForm((prev) => ({ ...prev, override_settings: value, ...tlsFields }));
  };

  const resetForm = () => {
    setForm(defaultForm);
  };

  const handleCreate = () => {
    let overrideSettings: Record<string, unknown> | undefined;
    try {
      overrideSettings = JSON.parse(form.override_settings);
    } catch {
      overrideSettings = undefined;
    }
    createMutation.mutate({
      node_id: form.node_id,
      protocol_config_id: form.protocol_config_id,
      is_active: form.is_active,
      override_settings: overrideSettings,
    });
  };

  const handleDelete = () => {
    if (!deleteBindingId) return;
    deleteMutation.mutate(deleteBindingId);
  };

  const openEdit = (binding: Binding) => {
    const override = binding.override_settings || {};
    setForm({
      node_id: binding.node_id,
      protocol_config_id: binding.protocol_config_id,
      is_active: binding.is_active,
      override_settings: binding.override_settings
        ? JSON.stringify(binding.override_settings, null, 2)
        : "{}",
      ...tlsFieldsFromOverride(override),
    });
    setEditBinding(binding);
  };

  const handleUpdate = () => {
    if (!editBinding) return;
    let overrideSettings: Record<string, unknown> | undefined;
    try {
      overrideSettings = JSON.parse(form.override_settings);
    } catch {
      overrideSettings = undefined;
    }
    updateMutation.mutate({
      id: editBinding.id,
      data: {
        is_active: form.is_active,
        override_settings: overrideSettings,
      },
    });
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("bindings.title")}
        action={{
          label: t("bindings.create"),
          onClick: () => {
            resetForm();
            setCreateOpen(true);
          },
        }}
      />

      <Card>
        <CardContent>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <BindingsTable
              bindings={bindings}
              nodes={nodes}
              protocols={protocols}
              onEdit={openEdit}
              onDelete={setDeleteBindingId}
            />
          )}
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
        title={t("bindings.deleteTitle")}
        isOpen={!!deleteBindingId}
        onClose={() => setDeleteBindingId(null)}
        onConfirm={handleDelete}
      >
        {t("bindings.deleteConfirm")}
      </ConfirmDialog>

      <BindingFormDialog
        mode="create"
        open={createOpen}
        form={form}
        nodes={nodes}
        protocols={protocols}
        certificates={certificates}
        onOpenChange={setCreateOpen}
        onFormChange={updateForm}
        onOverrideSettingsChange={setOverrideSettings}
        onSubmit={handleCreate}
      />

      <BindingFormDialog
        mode="edit"
        open={!!editBinding}
        form={form}
        nodes={nodes}
        protocols={protocols}
        certificates={certificates}
        onOpenChange={(open) => {
          if (!open) setEditBinding(null);
        }}
        onFormChange={updateForm}
        onOverrideSettingsChange={setOverrideSettings}
        onSubmit={handleUpdate}
      />
    </div>
  );
}
