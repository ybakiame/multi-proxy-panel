import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  Pagination,
  JsonEditor,
  FormInput,
  FormSelect,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getProtocols,
  createProtocol,
  updateProtocol,
  deleteProtocol,
  generateRealityKeys,
} from "../api/protocols";
import { ProtocolConfig } from "../api/types";

const PROTOCOL_TYPES = [
  "vless_reality",
  "vless_vision",
  "vless_xhttp",
  "vmess",
  "trojan",
  "shadowsocks2022",
  "hysteria2",
  "tuic",
  "anytls",
];

const CORE_TYPES = ["xray", "sing-box", "both"];
const FLOW_OPTIONS = ["xtls-rprx-vision"];
const XHTTP_MODES = ["auto", "packet-up", "stream-up"];
const SHADOWSOCKS2022_METHODS = [
  "2022-blake3-aes-128-gcm",
  "2022-blake3-aes-256-gcm",
  "2022-blake3-chacha20-poly1305",
];
const OBFS_TYPES = ["none", "salamander"];
const TUIC_CONGESTION = ["bbr", "cubic", "new_reno"];

interface ProtocolForm {
  name: string;
  protocol_type: string;
  core_type: string;
  core_version: string;
  listen_address: string;
  listen_port: string;
  tls_settings: string;
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
  congestion_control: string;
  alter_id: string;
  method: string;
}

const defaultForm: ProtocolForm = {
  name: "",
  protocol_type: "vless_reality",
  core_type: "xray",
  core_version: "",
  listen_address: "",
  listen_port: "",
  tls_settings: "{}",
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
  congestion_control: "bbr",
  alter_id: "",
  method: "2022-blake3-aes-256-gcm",
};

export function Protocols() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } =
    usePagination();
  const [protocols, setProtocols] = useState<ProtocolConfig[]>([]);
  const [loading, setLoading] = useState(false);
  const [isModalOpen, setIsModalOpen] = useState(false);
  const [editingProtocol, setEditingProtocol] = useState<ProtocolConfig | null>(
    null,
  );
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

  const parseSettings = (
    settings: Record<string, unknown>,
  ): Partial<ProtocolForm> => {
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
      down_mbps:
        settings.down_mbps !== undefined ? String(settings.down_mbps) : "",
      masquerade: (settings.masquerade as string) || "",
      congestion_control: (settings.congestion_control as string) || "bbr",
      alter_id:
        settings.alter_id !== undefined ? String(settings.alter_id) : "",
      method: (settings.method as string) || "2022-blake3-aes-256-gcm",
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
        core_version: protocol.core_version || "",
        listen_address: protocol.listen_address,
        listen_port: protocol.listen_port.toString(),
        tls_settings: protocol.tls_settings
          ? JSON.stringify(protocol.tls_settings, null, 2)
          : "{}",
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
      case "vless_vision":
        return {
          uuid: form.uuid,
          flow: form.flow,
        };
      case "vless_xhttp":
        return {
          uuid: form.uuid,
          path: form.path,
          host: form.host,
          mode: form.mode,
        };
      case "vmess":
        return {
          uuid: form.uuid,
          alter_id: Number(form.alter_id) || 0,
        };
      case "trojan":
        return {
          password: form.password,
        };
      case "shadowsocks2022":
        return {
          method: form.method,
          password: form.password,
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
      case "tuic":
        return {
          uuid: form.uuid,
          password: form.password,
          congestion_control: form.congestion_control,
        };
      default:
        return {};
    }
  };

  const buildPayload = () => {
    let tlsSettings: Record<string, unknown> | undefined;
    try {
      tlsSettings = JSON.parse(form.tls_settings);
    } catch {
      tlsSettings = undefined;
    }
    return {
      name: form.name,
      protocol_type: form.protocol_type,
      core_type: form.core_type,
      core_version: form.core_version || undefined,
      listen_address: form.listen_address,
      listen_port: Number(form.listen_port) || 0,
      settings: buildSettings(),
      tls_settings: tlsSettings,
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
              <Button
                className="self-end"
                variant="ghost"
                onPress={handleGenerateRealityKeys}
              >
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
      case "vless_vision":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => setForm({ ...form, uuid: value })}
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
          </>
        );
      case "vless_xhttp":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => setForm({ ...form, uuid: value })}
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
      case "vmess":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => setForm({ ...form, uuid: value })}
              isRequired
            />
            <FormInput
              type="number"
              label={t("protocols.alterId")}
              value={form.alter_id}
              onChange={(value) => setForm({ ...form, alter_id: value })}
            />
          </>
        );
      case "trojan":
        return (
          <FormInput
            label={t("protocols.password")}
            value={form.password}
            onChange={(value) => setForm({ ...form, password: value })}
            isRequired
          />
        );
      case "shadowsocks2022":
        return (
          <>
            <FormSelect
              label={t("protocols.method")}
              value={form.method}
              onChange={(value) => setForm({ ...form, method: value })}
              options={SHADOWSOCKS2022_METHODS.map((option) => ({
                id: option,
                label: option,
              }))}
            />
            <FormInput
              label={t("protocols.password")}
              value={form.password}
              onChange={(value) => setForm({ ...form, password: value })}
              isRequired
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
              isRequired
            />
            <FormInput
              label={t("protocols.masquerade")}
              value={form.masquerade}
              onChange={(value) => setForm({ ...form, masquerade: value })}
            />
          </>
        );
      case "tuic":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => setForm({ ...form, uuid: value })}
              isRequired
            />
            <FormInput
              label={t("protocols.password")}
              value={form.password}
              onChange={(value) => setForm({ ...form, password: value })}
              isRequired
            />
            <FormSelect
              label={t("protocols.congestionControl")}
              value={form.congestion_control}
              onChange={(value) =>
                setForm({ ...form, congestion_control: value })
              }
              options={TUIC_CONGESTION.map((option) => ({
                id: option,
                label: option,
              }))}
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
                            <Button
                              size="sm"
                              variant="ghost"
                              onPress={() => openEdit(protocol)}
                            >
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

      <Modal.Backdrop
        isOpen={isModalOpen}
        onOpenChange={(open) => setIsModalOpen(open)}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {editingProtocol
                  ? t("protocols.editTitle")
                  : t("protocols.createTitle")}
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
                label={t("protocols.type")}
                value={form.protocol_type}
                onChange={(value) => setForm({ ...form, protocol_type: value })}
                options={PROTOCOL_TYPES.map((type) => ({
                  id: type,
                  label: type,
                }))}
                isRequired
              />
              <FormSelect
                label={t("protocols.core")}
                value={form.core_type}
                onChange={(value) => setForm({ ...form, core_type: value })}
                options={CORE_TYPES.map((core) => ({ id: core, label: core }))}
                isRequired
              />
              <FormInput
                label={"Core Version"}
                value={form.core_version}
                onChange={(value) => setForm({ ...form, core_version: value })}
                placeholder="e.g. 1.14.0-beta.5 (leave empty for default)"
              />
              <FormInput
                label={t("protocols.listen")}
                value={form.listen_address}
                onChange={(value) =>
                  setForm({ ...form, listen_address: value })
                }
                isRequired
              />
              <FormInput
                type="number"
                label={t("protocols.port")}
                value={form.listen_port}
                onChange={(value) => setForm({ ...form, listen_port: value })}
                isRequired
              />
              <JsonEditor
                label={t("protocols.tlsSettings")}
                value={form.tls_settings}
                onChange={(value) => setForm({ ...form, tls_settings: value })}
              />
              {renderDynamicFields()}
            </Modal.Body>
            <Modal.Footer>
              <Button
                slot="close"
                variant="ghost"
                onPress={() => setIsModalOpen(false)}
              >
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
