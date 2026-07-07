import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
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
import { PageHeader, ConfirmDialog, Pagination, JsonEditor } from "../components/ui";
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
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [protocols, setProtocols] = useState<ProtocolConfig[]>([]);
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
      congestion_control: (settings.congestion_control as string) || "bbr",
      alter_id: settings.alter_id !== undefined ? String(settings.alter_id) : "",
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
            <Input
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(e) => setForm({ ...form, uuid: e.target.value })}
              isRequired
            />
            <Select
              label={t("protocols.flow")}
              selectedKeys={[form.flow]}
              onSelectionChange={(keys) =>
                setForm({ ...form, flow: Array.from(keys)[0] as string })
              }
            >
              {FLOW_OPTIONS.map((option) => (
                <SelectItem key={option}>{option}</SelectItem>
              ))}
            </Select>
            <Input
              label={t("protocols.dest")}
              value={form.dest}
              onChange={(e) => setForm({ ...form, dest: e.target.value })}
              isRequired
            />
            <Input
              label={t("protocols.serverNames")}
              value={form.server_names}
              onChange={(e) => setForm({ ...form, server_names: e.target.value })}
              placeholder="example.com, example.net"
              isRequired
            />
            <div className="flex gap-2">
              <Input
                className="flex-1"
                label={t("protocols.privateKey")}
                value={form.private_key}
                onChange={(e) => setForm({ ...form, private_key: e.target.value })}
              />
              <Button
                className="self-end"
                color="secondary"
                variant="flat"
                onPress={handleGenerateRealityKeys}
              >
                {t("protocols.generateKeys")}
              </Button>
            </div>
            <Input
              label={t("protocols.publicKey")}
              value={form.public_key}
              onChange={(e) => setForm({ ...form, public_key: e.target.value })}
            />
            <Input
              label={t("protocols.shortId")}
              value={form.short_id}
              onChange={(e) => setForm({ ...form, short_id: e.target.value })}
            />
          </>
        );
      case "vless_vision":
        return (
          <>
            <Input
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(e) => setForm({ ...form, uuid: e.target.value })}
              isRequired
            />
            <Select
              label={t("protocols.flow")}
              selectedKeys={[form.flow]}
              onSelectionChange={(keys) =>
                setForm({ ...form, flow: Array.from(keys)[0] as string })
              }
            >
              {FLOW_OPTIONS.map((option) => (
                <SelectItem key={option}>{option}</SelectItem>
              ))}
            </Select>
          </>
        );
      case "vless_xhttp":
        return (
          <>
            <Input
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(e) => setForm({ ...form, uuid: e.target.value })}
              isRequired
            />
            <Input
              label={t("protocols.path")}
              value={form.path}
              onChange={(e) => setForm({ ...form, path: e.target.value })}
            />
            <Input
              label={t("protocols.host")}
              value={form.host}
              onChange={(e) => setForm({ ...form, host: e.target.value })}
            />
            <Select
              label={t("protocols.mode")}
              selectedKeys={[form.mode]}
              onSelectionChange={(keys) =>
                setForm({ ...form, mode: Array.from(keys)[0] as string })
              }
            >
              {XHTTP_MODES.map((option) => (
                <SelectItem key={option}>{option}</SelectItem>
              ))}
            </Select>
          </>
        );
      case "vmess":
        return (
          <>
            <Input
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(e) => setForm({ ...form, uuid: e.target.value })}
              isRequired
            />
            <Input
              type="number"
              label={t("protocols.alterId")}
              value={form.alter_id}
              onChange={(e) => setForm({ ...form, alter_id: e.target.value })}
            />
          </>
        );
      case "trojan":
        return (
          <Input
            label={t("protocols.password")}
            value={form.password}
            onChange={(e) => setForm({ ...form, password: e.target.value })}
            isRequired
          />
        );
      case "shadowsocks2022":
        return (
          <>
            <Select
              label={t("protocols.method")}
              selectedKeys={[form.method]}
              onSelectionChange={(keys) =>
                setForm({ ...form, method: Array.from(keys)[0] as string })
              }
            >
              {SHADOWSOCKS2022_METHODS.map((option) => (
                <SelectItem key={option}>{option}</SelectItem>
              ))}
            </Select>
            <Input
              label={t("protocols.password")}
              value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })}
              isRequired
            />
          </>
        );
      case "hysteria2":
        return (
          <>
            <Input
              label={t("protocols.password")}
              value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })}
              isRequired
            />
            <Select
              label={t("protocols.obfsType")}
              selectedKeys={[form.obfs_type]}
              onSelectionChange={(keys) =>
                setForm({ ...form, obfs_type: Array.from(keys)[0] as string })
              }
            >
              {OBFS_TYPES.map((option) => (
                <SelectItem key={option}>{option}</SelectItem>
              ))}
            </Select>
            <Input
              label={t("protocols.obfsPassword")}
              value={form.obfs_password}
              onChange={(e) => setForm({ ...form, obfs_password: e.target.value })}
            />
            <Input
              type="number"
              label={t("protocols.upMbps")}
              value={form.up_mbps}
              onChange={(e) => setForm({ ...form, up_mbps: e.target.value })}
            />
            <Input
              type="number"
              label={t("protocols.downMbps")}
              value={form.down_mbps}
              onChange={(e) => setForm({ ...form, down_mbps: e.target.value })}
            />
          </>
        );
      case "anytls":
        return (
          <>
            <Input
              label={t("protocols.password")}
              value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })}
              isRequired
            />
            <Input
              label={t("protocols.masquerade")}
              value={form.masquerade}
              onChange={(e) => setForm({ ...form, masquerade: e.target.value })}
            />
          </>
        );
      case "tuic":
        return (
          <>
            <Input
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(e) => setForm({ ...form, uuid: e.target.value })}
              isRequired
            />
            <Input
              label={t("protocols.password")}
              value={form.password}
              onChange={(e) => setForm({ ...form, password: e.target.value })}
              isRequired
            />
            <Select
              label={t("protocols.congestionControl")}
              selectedKeys={[form.congestion_control]}
              onSelectionChange={(keys) =>
                setForm({ ...form, congestion_control: Array.from(keys)[0] as string })
              }
            >
              {TUIC_CONGESTION.map((option) => (
                <SelectItem key={option}>{option}</SelectItem>
              ))}
            </Select>
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
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table removeWrapper aria-label="protocols">
              <TableHeader>
                <TableColumn>{t("common.name")}</TableColumn>
                <TableColumn>{t("protocols.type")}</TableColumn>
                <TableColumn>{t("protocols.core")}</TableColumn>
                <TableColumn>{t("protocols.listen")}</TableColumn>
                <TableColumn>{t("protocols.port")}</TableColumn>
                <TableColumn>{t("common.actions")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {protocols.map((protocol) => (
                  <TableRow key={protocol.id}>
                    <TableCell>{protocol.name}</TableCell>
                    <TableCell>{protocol.protocol_type}</TableCell>
                    <TableCell>{protocol.core_type}</TableCell>
                    <TableCell>{protocol.listen_address}</TableCell>
                    <TableCell>{protocol.listen_port}</TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button size="sm" variant="flat" onPress={() => openEdit(protocol)}>
                          {t("common.edit")}
                        </Button>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteProtocolId(protocol.id)}
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
        title={t("protocols.deleteTitle")}
        isOpen={!!deleteProtocolId}
        onClose={() => setDeleteProtocolId(null)}
        onConfirm={handleDelete}
      >
        {t("protocols.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={isModalOpen} onClose={() => setIsModalOpen(false)}>
        <ModalContent>
          <ModalHeader>
            {editingProtocol ? t("protocols.editTitle") : t("protocols.createTitle")}
          </ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              isRequired
            />
            <Select
              label={t("protocols.type")}
              selectedKeys={[form.protocol_type]}
              onSelectionChange={(keys) =>
                setForm({ ...form, protocol_type: Array.from(keys)[0] as string })
              }
              isRequired
            >
              {PROTOCOL_TYPES.map((type) => (
                <SelectItem key={type}>{type}</SelectItem>
              ))}
            </Select>
            <Select
              label={t("protocols.core")}
              selectedKeys={[form.core_type]}
              onSelectionChange={(keys) =>
                setForm({ ...form, core_type: Array.from(keys)[0] as string })
              }
              isRequired
            >
              {CORE_TYPES.map((core) => (
                <SelectItem key={core}>{core}</SelectItem>
              ))}
            </Select>
            <Input
              label={t("protocols.listen")}
              value={form.listen_address}
              onChange={(e) => setForm({ ...form, listen_address: e.target.value })}
              isRequired
            />
            <Input
              type="number"
              label={t("protocols.port")}
              value={form.listen_port}
              onChange={(e) => setForm({ ...form, listen_port: e.target.value })}
              isRequired
            />
            <JsonEditor
              label={t("protocols.tlsSettings")}
              value={form.tls_settings}
              onChange={(value) => setForm({ ...form, tls_settings: value })}
            />
            {renderDynamicFields()}
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setIsModalOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button color="primary" onPress={editingProtocol ? handleUpdate : handleCreate}>
              {editingProtocol ? t("common.update") : t("common.create")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}
