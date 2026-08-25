import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  Pagination,
  JsonEditor,
  FormSelect,
  FormCheckbox,
  FormInput,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getBindingsPaginated, createBinding, updateBinding, deleteBinding } from "../api/bindings";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { getCertificates } from "../api/certificates";
import { Binding, ManagedCertificate } from "../api/types";

const bindingsQueryKey = "bindings";
const nodesQueryKey = "nodes";
const protocolsQueryKey = "protocols";
const certificatesQueryKey = "certificates";

interface BindingForm {
  node_id: string;
  protocol_config_id: string;
  is_active: boolean;
  override_settings: string;
  tls_type: "none" | "certificate" | "acme" | "managed";
  tls_cert_file: string;
  tls_key_file: string;
  tls_domain: string;
  tls_cert_id: string;
}

const defaultForm: BindingForm = {
  node_id: "",
  protocol_config_id: "",
  is_active: true,
  override_settings: "{}",
  tls_type: "none",
  tls_cert_file: "",
  tls_key_file: "",
  tls_domain: "",
  tls_cert_id: "",
};

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

  const selectedProtocol = protocols.find((p) => p.id === form.protocol_config_id);
  const showTlsOverride = !!selectedProtocol && selectedProtocol.protocol_type !== "vless_reality";
  const isSingBox = selectedProtocol?.core_type === "sing-box";

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

  const getNodeName = (nodeId: string) => {
    const node = nodes.find((n) => n.id === nodeId);
    return node ? node.name : nodeId;
  };

  const getProtocolName = (protocolId: string) => {
    const protocol = protocols.find((p) => p.id === protocolId);
    return protocol ? protocol.name : protocolId;
  };

  const renderTlsFields = () => {
    if (!showTlsOverride) return null;
    const tlsOptions = [
      { id: "none", label: t("protocols.tlsTypeNone") },
      { id: "managed", label: t("protocols.tlsTypeManaged") },
      { id: "certificate", label: t("protocols.tlsTypeCertificate") },
      ...(isSingBox ? [{ id: "acme", label: t("protocols.tlsTypeAcme") }] : []),
    ];
    return (
      <div className="space-y-4 rounded-lg border border-border p-4">
        <div className="text-sm font-medium">{t("bindings.tlsOverride")}</div>
        <FormSelect
          label={t("protocols.tlsType")}
          value={form.tls_type}
          onChange={(value) => updateForm({ tls_type: value as BindingForm["tls_type"] })}
          options={tlsOptions}
        />
        {form.tls_type === "managed" && (
          <FormSelect
            label={t("protocols.managedCert")}
            value={form.tls_cert_id}
            onChange={(value) => updateForm({ tls_cert_id: value })}
            options={certificates
              .filter((c) => c.status === "active" && c.node_id === form.node_id)
              .map((c) => ({ id: c.id, label: c.domain }))}
            isRequired
          />
        )}
        {form.tls_type === "certificate" && (
          <>
            <FormInput
              label={t("protocols.tlsCertFile")}
              value={form.tls_cert_file}
              onChange={(value) => updateForm({ tls_cert_file: value })}
              placeholder="/etc/ssl/certs/example.crt"
              isRequired
            />
            <FormInput
              label={t("protocols.tlsKeyFile")}
              value={form.tls_key_file}
              onChange={(value) => updateForm({ tls_key_file: value })}
              placeholder="/etc/ssl/private/example.key"
              isRequired
            />
          </>
        )}
        {form.tls_type === "acme" && isSingBox && (
          <FormInput
            label={t("protocols.tlsDomain")}
            value={form.tls_domain}
            onChange={(value) => updateForm({ tls_domain: value })}
            placeholder="hy2.example.com"
            description={t("protocols.tlsDomainDescription")}
            isRequired
          />
        )}
      </div>
    );
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
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="bindings">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("bindings.node")}</Table.Column>
                    <Table.Column>{t("bindings.protocol")}</Table.Column>
                    <Table.Column>{t("common.active")}</Table.Column>
                    <Table.Column>{t("bindings.overrideSettings")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </div>
                    )}
                  >
                    {bindings.map((binding) => (
                      <Table.Row key={binding.id}>
                        <Table.Cell>{getNodeName(binding.node_id)}</Table.Cell>
                        <Table.Cell>{getProtocolName(binding.protocol_config_id)}</Table.Cell>
                        <Table.Cell>
                          {binding.is_active ? t("common.enabled") : t("common.disabled")}
                        </Table.Cell>
                        <Table.Cell className="max-w-xs truncate font-mono">
                          {binding.override_settings
                            ? JSON.stringify(binding.override_settings)
                            : "-"}
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            <Button size="sm" variant="ghost" onPress={() => openEdit(binding)}>
                              {t("common.edit")}
                            </Button>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteBindingId(binding.id)}
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
        title={t("bindings.deleteTitle")}
        isOpen={!!deleteBindingId}
        onClose={() => setDeleteBindingId(null)}
        onConfirm={handleDelete}
      >
        {t("bindings.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("bindings.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("bindings.node")}
                value={form.node_id}
                onChange={(value) => updateForm({ node_id: value })}
                options={nodes.map((node) => ({
                  id: node.id,
                  label: node.name,
                }))}
                isRequired
              />
              <FormSelect
                label={t("bindings.protocol")}
                value={form.protocol_config_id}
                onChange={(value) => updateForm({ protocol_config_id: value })}
                options={protocols.map((protocol) => ({
                  id: protocol.id,
                  label: protocol.name,
                }))}
                isRequired
              />
              <FormCheckbox
                isSelected={form.is_active}
                onChange={(selected) => updateForm({ is_active: selected })}
              >
                {t("common.active")}
              </FormCheckbox>
              {renderTlsFields()}
              <JsonEditor
                label={t("bindings.overrideSettings")}
                value={form.override_settings}
                onChange={(value) => setOverrideSettings(value)}
              />
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
        isOpen={!!editBinding}
        onOpenChange={(open) => {
          if (!open) setEditBinding(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("bindings.editTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("bindings.node")}
                value={form.node_id}
                onChange={(value) => updateForm({ node_id: value })}
                options={nodes.map((node) => ({
                  id: node.id,
                  label: node.name,
                }))}
                isDisabled
              />
              <FormSelect
                label={t("bindings.protocol")}
                value={form.protocol_config_id}
                onChange={(value) => updateForm({ protocol_config_id: value })}
                options={protocols.map((protocol) => ({
                  id: protocol.id,
                  label: protocol.name,
                }))}
                isDisabled
              />
              <FormCheckbox
                isSelected={form.is_active}
                onChange={(selected) => updateForm({ is_active: selected })}
              >
                {t("common.active")}
              </FormCheckbox>
              {renderTlsFields()}
              <JsonEditor
                label={t("bindings.overrideSettings")}
                value={form.override_settings}
                onChange={(value) => setOverrideSettings(value)}
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setEditBinding(null)}>
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
