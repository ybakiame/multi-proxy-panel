import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Checkbox,
  Input,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  Spinner,
} from "@heroui/react";
import { PageHeader, ConfirmDialog, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getHosts, createHost, updateHost, deleteHost } from "../api/hosts";
import type { CreateHostPayload } from "../api/hosts";
import { InboundHost } from "../api/types";

interface HostForm {
  protocol_config_id: string;
  node_id: string;
  remark: string;
  address: string;
  port: string;
  sni: string;
  host: string;
  path: string;
  is_active: boolean;
}

export function Hosts() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [hosts, setHosts] = useState<InboundHost[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editHost, setEditHost] = useState<InboundHost | null>(null);
  const [deleteHostId, setDeleteHostId] = useState<string | null>(null);
  const [form, setForm] = useState<HostForm>({
    protocol_config_id: "",
    node_id: "",
    remark: "",
    address: "",
    port: "",
    sni: "",
    host: "",
    path: "",
    is_active: true,
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getHosts(page, perPage);
      setHosts(res.data);
      setTotal(res.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  const resetForm = (host?: InboundHost) => {
    if (host) {
      setForm({
        protocol_config_id: host.protocol_config_id,
        node_id: host.node_id,
        remark: host.remark,
        address: host.address,
        port: host.port.toString(),
        sni: host.sni || "",
        host: host.host || "",
        path: host.path || "",
        is_active: host.is_active,
      });
    } else {
      setForm({
        protocol_config_id: "",
        node_id: "",
        remark: "",
        address: "",
        port: "",
        sni: "",
        host: "",
        path: "",
        is_active: true,
      });
    }
  };

  const buildPayload = (): Partial<CreateHostPayload> => {
    return {
      protocol_config_id: form.protocol_config_id || undefined,
      node_id: form.node_id || undefined,
      remark: form.remark || undefined,
      address: form.address || undefined,
      port: form.port ? Number(form.port) : undefined,
      sni: form.sni || undefined,
      host: form.host || undefined,
      path: form.path || undefined,
      is_active: form.is_active,
    };
  };

  const handleCreate = async () => {
    try {
      await createHost({
        protocol_config_id: form.protocol_config_id,
        node_id: form.node_id,
        remark: form.remark,
        address: form.address,
        port: Number(form.port),
        sni: form.sni || undefined,
        host: form.host || undefined,
        path: form.path || undefined,
        is_active: form.is_active,
      });
      setCreateOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleUpdate = async () => {
    if (!editHost) return;
    try {
      await updateHost(editHost.id, buildPayload());
      setEditHost(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteHostId) return;
    try {
      await deleteHost(deleteHostId);
      setDeleteHostId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const openEdit = (host: InboundHost) => {
    resetForm(host);
    setEditHost(host);
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("hosts.title")}
        action={{
          label: t("hosts.create"),
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
            <Table removeWrapper aria-label="hosts">
              <TableHeader>
                <TableColumn>{t("hosts.remark")}</TableColumn>
                <TableColumn>{t("hosts.address")}</TableColumn>
                <TableColumn>{t("hosts.port")}</TableColumn>
                <TableColumn>{t("hosts.sni")}</TableColumn>
                <TableColumn>{t("hosts.host")}</TableColumn>
                <TableColumn>{t("hosts.path")}</TableColumn>
                <TableColumn>{t("hosts.isActive")}</TableColumn>
                <TableColumn>{t("hosts.protocolConfig")}</TableColumn>
                <TableColumn>{t("hosts.node")}</TableColumn>
                <TableColumn>{t("common.actions")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {hosts.map((host) => (
                  <TableRow key={host.id}>
                    <TableCell>{host.remark}</TableCell>
                    <TableCell>{host.address}</TableCell>
                    <TableCell>{host.port}</TableCell>
                    <TableCell>{host.sni || "-"}</TableCell>
                    <TableCell>{host.host || "-"}</TableCell>
                    <TableCell>{host.path || "-"}</TableCell>
                    <TableCell>{host.is_active ? t("common.enabled") : t("common.disabled")}</TableCell>
                    <TableCell className="max-w-xs truncate">{host.protocol_config_id}</TableCell>
                    <TableCell className="max-w-xs truncate">{host.node_id}</TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button size="sm" variant="flat" onPress={() => openEdit(host)}>
                          {t("common.edit")}
                        </Button>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteHostId(host.id)}
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
        title={t("hosts.deleteTitle")}
        isOpen={!!deleteHostId}
        onClose={() => setDeleteHostId(null)}
        onConfirm={handleDelete}
      >
        {t("hosts.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("hosts.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("hosts.protocolConfig")}
              value={form.protocol_config_id}
              onChange={(e) => setForm({ ...form, protocol_config_id: e.target.value })}
              placeholder="UUID"
              isRequired
            />
            <Input
              label={t("hosts.node")}
              value={form.node_id}
              onChange={(e) => setForm({ ...form, node_id: e.target.value })}
              placeholder="UUID"
              isRequired
            />
            <Input
              label={t("hosts.remark")}
              value={form.remark}
              onChange={(e) => setForm({ ...form, remark: e.target.value })}
              isRequired
            />
            <Input
              label={t("hosts.address")}
              value={form.address}
              onChange={(e) => setForm({ ...form, address: e.target.value })}
              isRequired
            />
            <Input
              type="number"
              label={t("hosts.port")}
              value={form.port}
              onChange={(e) => setForm({ ...form, port: e.target.value })}
              isRequired
            />
            <Input
              label={t("hosts.sni")}
              value={form.sni}
              onChange={(e) => setForm({ ...form, sni: e.target.value })}
            />
            <Input
              label={t("hosts.host")}
              value={form.host}
              onChange={(e) => setForm({ ...form, host: e.target.value })}
            />
            <Input
              label={t("hosts.path")}
              value={form.path}
              onChange={(e) => setForm({ ...form, path: e.target.value })}
            />
            <Checkbox
              isSelected={form.is_active}
              onValueChange={(selected) => setForm({ ...form, is_active: selected })}
            >
              {t("hosts.isActive")}
            </Checkbox>
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

      <Modal isOpen={!!editHost} onClose={() => setEditHost(null)}>
        <ModalContent>
          <ModalHeader>{t("hosts.editTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("hosts.protocolConfig")}
              value={form.protocol_config_id}
              onChange={(e) => setForm({ ...form, protocol_config_id: e.target.value })}
              placeholder="UUID"
            />
            <Input
              label={t("hosts.node")}
              value={form.node_id}
              onChange={(e) => setForm({ ...form, node_id: e.target.value })}
              placeholder="UUID"
            />
            <Input
              label={t("hosts.remark")}
              value={form.remark}
              onChange={(e) => setForm({ ...form, remark: e.target.value })}
            />
            <Input
              label={t("hosts.address")}
              value={form.address}
              onChange={(e) => setForm({ ...form, address: e.target.value })}
            />
            <Input
              type="number"
              label={t("hosts.port")}
              value={form.port}
              onChange={(e) => setForm({ ...form, port: e.target.value })}
            />
            <Input
              label={t("hosts.sni")}
              value={form.sni}
              onChange={(e) => setForm({ ...form, sni: e.target.value })}
            />
            <Input
              label={t("hosts.host")}
              value={form.host}
              onChange={(e) => setForm({ ...form, host: e.target.value })}
            />
            <Input
              label={t("hosts.path")}
              value={form.path}
              onChange={(e) => setForm({ ...form, path: e.target.value })}
            />
            <Checkbox
              isSelected={form.is_active}
              onValueChange={(selected) => setForm({ ...form, is_active: selected })}
            >
              {t("hosts.isActive")}
            </Checkbox>
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setEditHost(null)}>
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
