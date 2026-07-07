import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Checkbox,
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
import { getBindingsPaginated, createBinding, deleteBinding } from "../api/bindings";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { Binding, Node, ProtocolConfig } from "../api/types";

interface BindingForm {
  node_id: string;
  protocol_config_id: string;
  is_active: boolean;
  override_settings: string;
}

export function Bindings() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [bindings, setBindings] = useState<Binding[]>([]);
  const [nodes, setNodes] = useState<Node[]>([]);
  const [protocols, setProtocols] = useState<ProtocolConfig[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteBindingId, setDeleteBindingId] = useState<string | null>(null);
  const [form, setForm] = useState<BindingForm>({
    node_id: "",
    protocol_config_id: "",
    is_active: true,
    override_settings: "{}",
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const [bindingsRes, nodesRes, protocolsRes] = await Promise.allSettled([
        getBindingsPaginated(page, perPage),
        getNodes(),
        getAllProtocols(),
      ]);
      if (bindingsRes.status === "fulfilled") {
        setBindings(bindingsRes.value.data);
        setTotal(bindingsRes.value.pagination.total);
      }
      if (nodesRes.status === "fulfilled") {
        setNodes(nodesRes.value);
      }
      if (protocolsRes.status === "fulfilled") {
        setProtocols(protocolsRes.value);
      }
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  const resetForm = () => {
    setForm({
      node_id: "",
      protocol_config_id: "",
      is_active: true,
      override_settings: "{}",
    });
  };

  const handleCreate = async () => {
    try {
      let overrideSettings: Record<string, unknown> | undefined;
      try {
        overrideSettings = JSON.parse(form.override_settings);
      } catch {
        overrideSettings = undefined;
      }
      await createBinding({
        node_id: form.node_id,
        protocol_config_id: form.protocol_config_id,
        is_active: form.is_active,
        override_settings: overrideSettings,
      });
      setCreateOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteBindingId) return;
    try {
      await deleteBinding(deleteBindingId);
      setDeleteBindingId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const getNodeName = (nodeId: string) => {
    const node = nodes.find((n) => n.id === nodeId);
    return node ? node.name : nodeId;
  };

  const getProtocolName = (protocolId: string) => {
    const protocol = protocols.find((p) => p.id === protocolId);
    return protocol ? protocol.name : protocolId;
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
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table removeWrapper aria-label="bindings">
              <TableHeader>
                <TableColumn>{t("bindings.node")}</TableColumn>
                <TableColumn>{t("bindings.protocol")}</TableColumn>
                <TableColumn>{t("common.active")}</TableColumn>
                <TableColumn>{t("bindings.overrideSettings")}</TableColumn>
                <TableColumn>{t("common.actions")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {bindings.map((binding) => (
                  <TableRow key={binding.id}>
                    <TableCell>{getNodeName(binding.node_id)}</TableCell>
                    <TableCell>{getProtocolName(binding.protocol_config_id)}</TableCell>
                    <TableCell>
                      {binding.is_active ? t("common.enabled") : t("common.disabled")}
                    </TableCell>
                    <TableCell className="max-w-xs truncate font-mono">
                      {binding.override_settings
                        ? JSON.stringify(binding.override_settings)
                        : "-"}
                    </TableCell>
                    <TableCell>
                      <Button
                        size="sm"
                        color="danger"
                        variant="flat"
                        onPress={() => setDeleteBindingId(binding.id)}
                      >
                        {t("common.delete")}
                      </Button>
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
        title={t("bindings.deleteTitle")}
        isOpen={!!deleteBindingId}
        onClose={() => setDeleteBindingId(null)}
        onConfirm={handleDelete}
      >
        {t("bindings.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("bindings.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Select
              label={t("bindings.node")}
              selectedKeys={form.node_id ? [form.node_id] : []}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, node_id: value || "" });
              }}
              isRequired
            >
              {nodes.map((node) => (
                <SelectItem key={node.id}>{node.name}</SelectItem>
              ))}
            </Select>
            <Select
              label={t("bindings.protocol")}
              selectedKeys={form.protocol_config_id ? [form.protocol_config_id] : []}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, protocol_config_id: value || "" });
              }}
              isRequired
            >
              {protocols.map((protocol) => (
                <SelectItem key={protocol.id}>{protocol.name}</SelectItem>
              ))}
            </Select>
            <Checkbox
              isSelected={form.is_active}
              onValueChange={(selected) => setForm({ ...form, is_active: selected })}
            >
              {t("common.active")}
            </Checkbox>
            <JsonEditor
              label={t("bindings.overrideSettings")}
              value={form.override_settings}
              onChange={(value) => setForm({ ...form, override_settings: value })}
            />
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
    </div>
  );
}
