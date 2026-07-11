import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  CopyableSecret,
  StatusBadge,
  FormInput,
  FormTextArea,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getNodesPaginated,
  createNode,
  updateNode,
  deleteNode,
  pushConfig,
  getNodeLogs,
} from "../api/nodes";
import { Node, AgentLog } from "../api/types";

export function Nodes() {
  const { t } = useTranslation();
  const { page, perPage, setTotal } = usePagination();
  const [nodes, setNodes] = useState<Node[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editNode, setEditNode] = useState<Node | null>(null);
  const [deleteNodeId, setDeleteNodeId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [logNode, setLogNode] = useState<Node | null>(null);
  const [nodeLogs, setNodeLogs] = useState<AgentLog[]>([]);
  const [logsLoading, setLogsLoading] = useState(false);
  const [form, setForm] = useState({
    name: "",
    hostname: "",
    address: "",
    usage_coefficient: 1,
    labels: "{}",
    parent_id: "",
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const nodesRes = await getNodesPaginated(page, perPage);
      setNodes(nodesRes.data);
      setTotal(nodesRes.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  const resetForm = (node?: Node) => {
    if (node) {
      setForm({
        name: node.name,
        hostname: node.hostname,
        address: node.address,
        usage_coefficient: node.usage_coefficient,
        labels: JSON.stringify(node.labels || {}),
        parent_id: node.parent_id || "",
      });
    } else {
      setForm({
        name: "",
        hostname: "",
        address: "",
        usage_coefficient: 1,
        labels: "{}",
        parent_id: "",
      });
    }
  };

  const handleCreate = async () => {
    try {
      let labels: Record<string, string> = {};
      try {
        labels = JSON.parse(form.labels);
      } catch {}
      const res = await createNode({
        name: form.name,
        hostname: form.hostname,
        address: form.address,
        usage_coefficient: form.usage_coefficient,
        labels,
        parent_id: form.parent_id || undefined,
      });
      setNewToken(res.token || null);
      setCreateOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleUpdate = async () => {
    if (!editNode) return;
    try {
      let labels: Record<string, string> = {};
      try {
        labels = JSON.parse(form.labels);
      } catch {}
      await updateNode(editNode.id, {
        name: form.name,
        hostname: form.hostname,
        address: form.address,
        usage_coefficient: form.usage_coefficient,
        labels,
        parent_id: form.parent_id || undefined,
      });
      setEditNode(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteNodeId) return;
    try {
      await deleteNode(deleteNodeId);
      setDeleteNodeId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handlePush = async (node: Node) => {
    try {
      await pushConfig(node.id, {
        core_type: "sing-box",
        restart: true,
        version: "1",
      });
    } catch {
      // error handled by axios interceptor
    }
  };

  const openEdit = (node: Node) => {
    resetForm(node);
    setEditNode(node);
  };

  const openLogs = async (node: Node) => {
    setLogNode(node);
    setLogsLoading(true);
    try {
      const res = await getNodeLogs(node.id, 100);
      setNodeLogs(res);
    } finally {
      setLogsLoading(false);
    }
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("nodes.title")}
        action={{
          label: t("nodes.create"),
          onClick: () => {
            resetForm();
            setNewToken(null);
            setCreateOpen(true);
          },
        }}
      />

      {newToken && <CopyableSecret secret={newToken} label={t("nodes.tokenWarning")} />}

      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="nodes">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("nodes.name")}</Table.Column>
                    <Table.Column>{t("nodes.hostname")}</Table.Column>
                    <Table.Column>{t("nodes.address")}</Table.Column>
                    <Table.Column>{t("common.status")}</Table.Column>
                    <Table.Column>{t("nodes.parentId")}</Table.Column>
                    <Table.Column>{t("nodes.cores")}</Table.Column>
                    <Table.Column>{t("nodes.usageCoefficient")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </div>
                    )}
                  >
                    {nodes.map((node) => (
                      <Table.Row key={node.id}>
                        <Table.Cell>{node.name}</Table.Cell>
                        <Table.Cell>{node.hostname}</Table.Cell>
                        <Table.Cell>{node.address}</Table.Cell>
                        <Table.Cell>
                          <StatusBadge status={node.status} />
                        </Table.Cell>
                        <Table.Cell>
                          {node.parent_id ? `${t("nodes.childOf")} ${node.parent_id}` : "-"}
                        </Table.Cell>
                        <Table.Cell>{(node.cores_available || []).join(", ")}</Table.Cell>
                        <Table.Cell>{node.usage_coefficient}</Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            <Button size="sm" variant="ghost" onPress={() => openEdit(node)}>
                              {t("common.edit")}
                            </Button>
                            <Button size="sm" variant="ghost" onPress={() => openLogs(node)}>
                              {t("nodes.logs")}
                            </Button>
                            <Button size="sm" variant="ghost" onPress={() => handlePush(node)}>
                              {t("nodes.pushConfig")}
                            </Button>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteNodeId(node.id)}
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

      <ConfirmDialog
        title={t("nodes.deleteTitle")}
        isOpen={!!deleteNodeId}
        onClose={() => setDeleteNodeId(null)}
        onConfirm={handleDelete}
      >
        {t("nodes.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("nodes.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("nodes.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormInput
                label={t("nodes.hostname")}
                value={form.hostname}
                onChange={(value) => setForm({ ...form, hostname: value })}
              />
              <FormInput
                label={t("nodes.address")}
                value={form.address}
                onChange={(value) => setForm({ ...form, address: value })}
              />
              <FormInput
                type="number"
                label={t("nodes.usageCoefficient")}
                value={form.usage_coefficient.toString()}
                onChange={(value) => setForm({ ...form, usage_coefficient: Number(value) })}
              />
              <FormInput
                label={t("nodes.parentId")}
                value={form.parent_id}
                onChange={(value) => setForm({ ...form, parent_id: value })}
                placeholder="UUID (optional)"
              />
              <FormTextArea
                label={t("nodes.labels")}
                value={form.labels}
                onChange={(value) => setForm({ ...form, labels: value })}
                className="font-mono"
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
        isOpen={!!editNode}
        onOpenChange={(open) => {
          if (!open) setEditNode(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("nodes.editTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("nodes.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
              />
              <FormInput
                label={t("nodes.hostname")}
                value={form.hostname}
                onChange={(value) => setForm({ ...form, hostname: value })}
              />
              <FormInput
                label={t("nodes.address")}
                value={form.address}
                onChange={(value) => setForm({ ...form, address: value })}
              />
              <FormInput
                type="number"
                label={t("nodes.usageCoefficient")}
                value={form.usage_coefficient.toString()}
                onChange={(value) => setForm({ ...form, usage_coefficient: Number(value) })}
              />
              <FormInput
                label={t("nodes.parentId")}
                value={form.parent_id}
                onChange={(value) => setForm({ ...form, parent_id: value })}
                placeholder="UUID (clear to remove)"
              />
              <FormTextArea
                label={t("nodes.labels")}
                value={form.labels}
                onChange={(value) => setForm({ ...form, labels: value })}
                className="font-mono"
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setEditNode(null)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleUpdate}>{t("common.update")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
      <Modal.Backdrop
        isOpen={!!logNode}
        onOpenChange={(open) => {
          if (!open) setLogNode(null);
        }}
      >
        <Modal.Container className="max-w-4xl">
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {logNode ? `${t("nodes.logs")}: ${logNode.name}` : t("nodes.logs")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="max-h-[60vh] overflow-auto space-y-2">
              {logsLoading ? (
                <div className="flex h-32 items-center justify-center">
                  <Spinner />
                </div>
              ) : nodeLogs.length === 0 ? (
                <div className="p-4 text-center text-muted-foreground">{t("common.empty")}</div>
              ) : (
                nodeLogs.map((log) => (
                  <div key={log.id} className="border-b border-divider pb-2 text-sm">
                    <div className="flex items-center gap-2">
                      <span
                        className={`rounded px-1.5 py-0.5 text-xs font-medium ${
                          log.level === "error"
                            ? "bg-danger-100 text-danger-700"
                            : log.level === "warn"
                              ? "bg-warning-100 text-warning-700"
                              : "bg-default-100 text-default-700"
                        }`}
                      >
                        {log.level}
                      </span>
                      <span className="text-muted-foreground">{log.target}</span>
                      <span className="ml-auto text-xs text-muted-foreground">
                        {new Date(log.created_at).toLocaleString()}
                      </span>
                    </div>
                    <p className="mt-1 whitespace-pre-wrap break-words">{log.message}</p>
                  </div>
                ))
              )}
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" onPress={() => setLogNode(null)}>
                {t("common.close")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
