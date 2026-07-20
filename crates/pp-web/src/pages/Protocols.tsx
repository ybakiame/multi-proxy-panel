import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import { PageHeader, ConfirmDialog, Pagination, FormInput, FormSelect } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getProtocols,
  createProtocol,
  updateProtocol,
  deleteProtocol,
  generateRealityKeys,
} from "../api/protocols";
import { getCoreVersions } from "../api/coreVersions";
import { ProtocolConfig, CoreVersion } from "../api/types";

const PROTOCOL_TYPES = ["vless_reality", "vless_xhttp", "hysteria2", "anytls"];

// Mirrors CoreType::valid_for() in crates/pp-common/src/protocol.rs
const CORE_PROTOCOLS: Record<string, string[]> = {
  xray: ["vless_reality", "vless_xhttp"],
  "sing-box": ["vless_reality", "hysteria2", "anytls"],
  mihomo: ["vless_reality", "vless_xhttp", "hysteria2", "anytls"],
};

const CORE_TYPES = ["xray", "sing-box", "mihomo"];
const FLOW_OPTIONS = ["xtls-rprx-vision"];
const XHTTP_MODES = ["auto", "packet-up", "stream-up"];
const OBFS_TYPES = ["none", "salamander"];

interface ProtocolForm {
  name: string;
  protocol_type: string;
  core_type: string;
  core_version: string;
  listen_address: string;
  listen_port: string;
  tls_type: "none" | "certificate" | "acme";
  tls_cert_file: string;
  tls_key_file: string;
  tls_domain: string;
  uuid: string;
  password: string;
  flow: string;
  dest: string;
  server_names: string;
  private_key: string;
  public_key: string;
  short_id: string;
  path: string;
  host: string;
  mode: string;
  obfs_type: string;
  obfs_password: string;
  up_mbps: string;
  down_mbps: string;
  masquerade: string;
}

const defaultForm: ProtocolForm = {
  name: "",
  protocol_type: "vless_reality",
  core_type: "xray",
  core_version: "",
  listen_address: "0.0.0.0",
  listen_port: "",
  tls_type: "none",
  tls_cert_file: "",
  tls_key_file: "",
  tls_domain: "",
  uuid: "",
  password: "",
  flow: "xtls-rprx-vision",
  dest: "",
  server_names: "",
  private_key: "",
  public_key: "",
  short_id: "",
  path: "",
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
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [protocols, setProtocols] = useState<ProtocolConfig[]>([]);
  const [coreVersions, setCoreVersions] = useState<CoreVersion[]>([]);
  const [loading, setLoading] = useState(false);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingProtocol, setEditingProtocol] = useState<ProtocolConfig | null>(null);
  const [deleteProtocolId, setDeleteProtocolId] = useState<string | null>(null);
  const [form, setForm] = useState<ProtocolForm>(defaultForm);

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getProtocols(page, perPage);
      setProtocols(res.data);
      setTotal(res.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  useEffect(() => {
    getCoreVersions()
      .then(setCoreVersions)
      .catch(() => {});
  }, []);

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

  const parseTlsSettings = (
    tls?: Record<string, unknown> | null,
  ): Partial<Pick<ProtocolForm, "tls_type" | "tls_cert_file" | "tls_key_file" | "tls_domain">> => {
    if (!tls) return { tls_type: "none" };
    const certFile = (tls.certFile as string) || "";
    const keyFile = (tls.keyFile as string) || "";
    const domain = (tls.domain as string) || "";
    if (domain) return { tls_type: "acme", tls_domain: domain };
    if (certFile || keyFile) {
      return { tls_type: "certificate", tls_cert_file: certFile, tls_key_file: keyFile };
    }
    return { tls_type: "none" };
  };

  const resetForm = (protocol?: ProtocolConfig) => {
    if (protocol) {
      setForm({
        ...defaultForm,
        ...parseSettings(protocol.settings || {}),
        ...parseTlsSettings(protocol.tls_settings),
        name: protocol.name,
        protocol_type: protocol.protocol_type,
        core_type: protocol.core_type,
        core_version: protocol.core_version || "",
        listen_address: protocol.listen_address,
        listen_port: protocol.listen_port.toString(),
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
    switch (form.tls_type) {
      case "certificate":
        if (!form.tls_cert_file.trim() || !form.tls_key_file.trim()) return undefined;
        return {
          certFile: form.tls_cert_file.trim(),
          keyFile: form.tls_key_file.trim(),
        };
      case "acme":
        if (!form.tls_domain.trim()) return undefined;
        return { domain: form.tls_domain.trim() };
      default:
        return undefined;
    }
  };

  const buildPayload = () => {
    return {
      name: form.name,
      protocol_type: form.protocol_type,
      core_type: form.core_type,
      core_version: form.core_version || undefined,
      listen_address: form.listen_address,
      listen_port: Number(form.listen_port) || 0,
      settings: buildSettings(),
      tls_settings: buildTlsSettings(),
    };
  };

  const handleCreate = async () => {
    try {
      await createProtocol(buildPayload());
      setIsModalOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleUpdate = async () => {
    if (!editingProtocol) return;
    try {
      await updateProtocol(editingProtocol.id, buildPayload());
      setIsModalOpen(false);
      setEditingProtocol(null);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteProtocolId) return;
    try {
      await deleteProtocol(deleteProtocolId);
      setDeleteProtocolId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleGenerateRealityKeys = async () => {
    try {
      const keys = await generateRealityKeys();
      setForm({
        ...form,
        private_key: keys.private_key || "",
        public_key: keys.public_key || "",
        short_id: keys.short_id || "",
      });
    } catch {
      // error handled by axios interceptor
    }
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

  const renderDynamicFields = () => {
    switch (form.protocol_type) {
      case "vless_reality":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => setForm({ ...form, uuid: value })}
              description={t("protocols.uuidDescription")}
              isRequired
            />
            <FormSelect
              label={t("protocols.flow")}
              value={form.flow}
              onChange={(value) => setForm({ ...form, flow: value })}
              options={FLOW_OPTIONS.map((option) => ({
                id: option,
                label: option,
              }))}
            />
            <FormInput
              label={t("protocols.dest")}
              value={form.dest}
              onChange={(value) => setForm({ ...form, dest: value })}
              isRequired
            />
            <FormInput
              label={t("protocols.serverNames")}
              value={form.server_names}
              onChange={(value) => setForm({ ...form, server_names: value })}
              placeholder="example.com, example.net"
              isRequired
            />
            <div className="flex gap-2">
              <FormInput
                className="flex-1"
                label={t("protocols.privateKey")}
                value={form.private_key}
                onChange={(value) => setForm({ ...form, private_key: value })}
              />
              <Button className="self-end" variant="ghost" onPress={handleGenerateRealityKeys}>
                {t("protocols.generateKeys")}
              </Button>
            </div>
            <FormInput
              label={t("protocols.publicKey")}
              value={form.public_key}
              onChange={(value) => setForm({ ...form, public_key: value })}
            />
            <FormInput
              label={t("protocols.shortId")}
              value={form.short_id}
              onChange={(value) => setForm({ ...form, short_id: value })}
            />
          </>
        );
      case "vless_xhttp":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => setForm({ ...form, uuid: value })}
              description={t("protocols.uuidDescription")}
              isRequired
            />
            <FormInput
              label={t("protocols.path")}
              value={form.path}
              onChange={(value) => setForm({ ...form, path: value })}
            />
            <FormInput
              label={t("protocols.host")}
              value={form.host}
              onChange={(value) => setForm({ ...form, host: value })}
            />
            <FormSelect
              label={t("protocols.mode")}
              value={form.mode}
              onChange={(value) => setForm({ ...form, mode: value })}
              options={XHTTP_MODES.map((option) => ({
                id: option,
                label: option,
              }))}
            />
          </>
        );
      case "hysteria2":
        return (
          <>
            <FormInput
              label={t("protocols.password")}
              value={form.password}
              onChange={(value) => setForm({ ...form, password: value })}
              description={t("protocols.passwordDescription")}
              isRequired
            />
            <FormSelect
              label={t("protocols.obfsType")}
              value={form.obfs_type}
              onChange={(value) => setForm({ ...form, obfs_type: value })}
              options={OBFS_TYPES.map((option) => ({
                id: option,
                label: option,
              }))}
            />
            <FormInput
              label={t("protocols.obfsPassword")}
              value={form.obfs_password}
              onChange={(value) => setForm({ ...form, obfs_password: value })}
            />
            <FormInput
              type="number"
              label={t("protocols.upMbps")}
              value={form.up_mbps}
              onChange={(value) => setForm({ ...form, up_mbps: value })}
            />
            <FormInput
              type="number"
              label={t("protocols.downMbps")}
              value={form.down_mbps}
              onChange={(value) => setForm({ ...form, down_mbps: value })}
            />
          </>
        );
      case "anytls":
        return (
          <>
            <FormInput
              label={t("protocols.password")}
              value={form.password}
              onChange={(value) => setForm({ ...form, password: value })}
              description={t("protocols.passwordDescription")}
              isRequired
            />
            <FormInput
              label={t("protocols.masquerade")}
              value={form.masquerade}
              onChange={(value) => setForm({ ...form, masquerade: value })}
            />
          </>
        );
      default:
        return null;
    }
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
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="protocols">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("common.name")}</Table.Column>
                    <Table.Column>{t("protocols.type")}</Table.Column>
                    <Table.Column>{t("protocols.core")}</Table.Column>
                    <Table.Column>Version</Table.Column>
                    <Table.Column>{t("protocols.listen")}</Table.Column>
                    <Table.Column>{t("protocols.port")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </div>
                    )}
                  >
                    {protocols.map((protocol) => (
                      <Table.Row key={protocol.id}>
                        <Table.Cell>{protocol.name}</Table.Cell>
                        <Table.Cell>{protocol.protocol_type}</Table.Cell>
                        <Table.Cell>{protocol.core_type}</Table.Cell>
                        <Table.Cell>{protocol.core_version || "-"}</Table.Cell>
                        <Table.Cell>{protocol.listen_address}</Table.Cell>
                        <Table.Cell>{protocol.listen_port}</Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            <Button size="sm" variant="ghost" onPress={() => openEdit(protocol)}>
                              {t("common.edit")}
                            </Button>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteProtocolId(protocol.id)}
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
        title={t("protocols.deleteTitle")}
        isOpen={!!deleteProtocolId}
        onClose={() => setDeleteProtocolId(null)}
        onConfirm={handleDelete}
      >
        {t("protocols.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={isModalOpen} onOpenChange={(open) => setIsModalOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {editingProtocol ? t("protocols.editTitle") : t("protocols.createTitle")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormSelect
                label={t("protocols.core")}
                value={form.core_type}
                onChange={(value) => {
                  const supported = CORE_PROTOCOLS[value] ?? PROTOCOL_TYPES;
                  setForm({
                    ...form,
                    core_type: value,
                    protocol_type: supported.includes(form.protocol_type)
                      ? form.protocol_type
                      : supported[0],
                  });
                }}
                options={CORE_TYPES.map((core) => ({ id: core, label: core }))}
                isRequired
              />
              <FormSelect
                label={t("protocols.type")}
                value={form.protocol_type}
                onChange={(value) => setForm({ ...form, protocol_type: value })}
                options={(CORE_PROTOCOLS[form.core_type] ?? PROTOCOL_TYPES).map((type) => ({
                  id: type,
                  label: type,
                }))}
                isRequired
              />
              <FormSelect
                label={t("protocols.coreVersion")}
                value={form.core_version || "__default__"}
                onChange={(value) =>
                  setForm({ ...form, core_version: value === "__default__" ? "" : value })
                }
                options={[
                  { id: "__default__", label: t("protocols.coreVersionDefault") },
                  ...coreVersions
                    .filter((v) => v.core_type === form.core_type)
                    .map((v) => ({
                      id: v.version,
                      label:
                        v.channel === "prerelease"
                          ? `${v.version} (${t("coreVersions.prerelease")})`
                          : v.version,
                    })),
                ]}
              />
              <FormInput
                label={t("protocols.listen")}
                value={form.listen_address}
                onChange={(value) => setForm({ ...form, listen_address: value })}
                isRequired
              />
              <div className="flex gap-2">
                <FormInput
                  className="flex-1"
                  type="number"
                  label={t("protocols.port")}
                  value={form.listen_port}
                  onChange={(value) => setForm({ ...form, listen_port: value })}
                  isRequired
                />
                <Button
                  className="self-end"
                  variant="ghost"
                  onPress={() => setForm({ ...form, listen_port: String(generateRandomPort()) })}
                >
                  {t("protocols.randomPort")}
                </Button>
              </div>
              <FormSelect
                label={t("protocols.tlsType")}
                value={form.tls_type}
                onChange={(value) =>
                  setForm({
                    ...form,
                    tls_type: value as ProtocolForm["tls_type"],
                    tls_cert_file: "",
                    tls_key_file: "",
                    tls_domain: "",
                  })
                }
                options={[
                  { id: "none", label: t("protocols.tlsTypeNone") },
                  { id: "certificate", label: t("protocols.tlsTypeCertificate") },
                  { id: "acme", label: t("protocols.tlsTypeAcme") },
                ]}
              />
              {form.tls_type === "certificate" && (
                <>
                  <FormInput
                    label={t("protocols.tlsCertFile")}
                    value={form.tls_cert_file}
                    onChange={(value) => setForm({ ...form, tls_cert_file: value })}
                    placeholder="/etc/ssl/certs/example.crt"
                    isRequired
                  />
                  <FormInput
                    label={t("protocols.tlsKeyFile")}
                    value={form.tls_key_file}
                    onChange={(value) => setForm({ ...form, tls_key_file: value })}
                    placeholder="/etc/ssl/private/example.key"
                    isRequired
                  />
                </>
              )}
              {form.tls_type === "acme" && (
                <FormInput
                  label={t("protocols.tlsDomain")}
                  value={form.tls_domain}
                  onChange={(value) => setForm({ ...form, tls_domain: value })}
                  placeholder="hy2.example.com"
                  description={t("protocols.tlsDomainDescription")}
                  isRequired
                />
              )}
              {renderDynamicFields()}
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setIsModalOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={editingProtocol ? handleUpdate : handleCreate}>
                {editingProtocol ? t("common.update") : t("common.create")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
