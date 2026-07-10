import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  Pagination,
  JsonEditor,
  FormSelect,
  FormCheckbox,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getBindingsPaginated,
  createBinding,
  deleteBinding,
} from "../api/bindings";
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
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } =
    usePagination();
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
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table>
              <Table.ScrollContainer>
                <Table.Content aria-label="bindings">
                  <Table.Header>
                    <Table.Column isRowHeader>
                      {t("bindings.node")}
                    </Table.Column>
                    <Table.Column>{t("bindings.protocol")}</Table.Column>
                    <Table.Column>{t("common.active")}</Table.Column>
                    <Table.Column>
                      {t("bindings.overrideSettings")}
                    </Table.Column>
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
                        <Table.Cell>
                          {getProtocolName(binding.protocol_config_id)}
                        </Table.Cell>
                        <Table.Cell>
                          {binding.is_active
                            ? t("common.enabled")
                            : t("common.disabled")}
                        </Table.Cell>
                        <Table.Cell className="max-w-xs truncate font-mono">
                          {binding.override_settings
                            ? JSON.stringify(binding.override_settings)
                            : "-"}
                        </Table.Cell>
                        <Table.Cell>
                          <Button
                            size="sm"
                            variant="danger"
                            onPress={() => setDeleteBindingId(binding.id)}
                          >
                            {t("common.delete")}
                          </Button>
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

      <Modal.Backdrop
        isOpen={createOpen}
        onOpenChange={(open) => setCreateOpen(open)}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("bindings.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("bindings.node")}
                value={form.node_id}
                onChange={(value) => setForm({ ...form, node_id: value })}
                options={nodes.map((node) => ({
                  id: node.id,
                  label: node.name,
                }))}
                isRequired
              />
              <FormSelect
                label={t("bindings.protocol")}
                value={form.protocol_config_id}
                onChange={(value) =>
                  setForm({ ...form, protocol_config_id: value })
                }
                options={protocols.map((protocol) => ({
                  id: protocol.id,
                  label: protocol.name,
                }))}
                isRequired
              />
              <FormCheckbox
                isSelected={form.is_active}
                onChange={(selected) =>
                  setForm({ ...form, is_active: selected })
                }
              >
                {t("common.active")}
              </FormCheckbox>
              <JsonEditor
                label={t("bindings.overrideSettings")}
                value={form.override_settings}
                onChange={(value) =>
                  setForm({ ...form, override_settings: value })
                }
              />
            </Modal.Body>
            <Modal.Footer>
              <Button
                slot="close"
                variant="ghost"
                onPress={() => setCreateOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button onPress={handleCreate}>{t("common.create")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
