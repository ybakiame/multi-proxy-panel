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
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  Textarea,
  Checkbox,
  Spinner,
  Select,
  SelectItem,
} from "@heroui/react";
import { PageHeader, ConfirmDialog, CopyableSecret, StatusBadge } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getNodesPaginated, createNode, updateNode, deleteNode, pushConfig } from "../api/nodes";
import { getGroups } from "../api/groups";
import { Node, Group } from "../api/types";
import { formatDateTime } from "../utils/format";

const CORE_OPTIONS = [
  { key: "xray", label: "Xray" },
  { key: "sing-box", label: "Sing-box" },
];

export function Nodes() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [nodes, setNodes] = useState<Node[]>([]);
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editNode, setEditNode] = useState<Node | null>(null);
  const [deleteNodeId, setDeleteNodeId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    hostname: "",
    address: "",
    usage_coefficient: 1,
    labels: "{}",
    parent_id: "",
    selectedGroups: new Set<string>(),
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const [nodesRes, groupsRes] = await Promise.all([
        getNodesPaginated(page, perPage),
        getGroups(),
      ]);
      setNodes(nodesRes.data);
      setTotal(nodesRes.pagination.total);
      setGroups(groupsRes);
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
        selectedGroups: new Set(node.group_ids || []),
      });
    } else {
      setForm({
        name: "",
        hostname: "",
        address: "",
        usage_coefficient: 1,
        labels: "{}",
        parent_id: "",
        selectedGroups: new Set<string>(),
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
        group_ids: Array.from(form.selectedGroups),
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
        group_ids: Array.from(form.selectedGroups),
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

      {newToken && (
        <CopyableSecret
          secret={newToken}
          label={t("nodes.tokenWarning")}
        />
      )}

      <Card>
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table removeWrapper aria-label="nodes">
              <TableHeader>
                <TableColumn>{t("nodes.name")}</TableColumn>
                <TableColumn>{t("nodes.hostname")}</TableColumn>
                <TableColumn>{t("nodes.address")}</TableColumn>
                <TableColumn>{t("common.status")}</TableColumn>
                <TableColumn>{t("nodes.parentId")}</TableColumn>
                <TableColumn>{t("nodes.cores")}</TableColumn>
                <TableColumn>{t("nodes.usageCoefficient")}</TableColumn>
                <TableColumn>{t("common.actions")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {nodes.map((node) => (
                  <TableRow key={node.id}>
                    <TableCell>{node.name}</TableCell>
                    <TableCell>{node.hostname}</TableCell>
                    <TableCell>{node.address}</TableCell>
                    <TableCell>
                      <StatusBadge status={node.status} />
                    </TableCell>
                    <TableCell>
                      {node.parent_id
                        ? `${t("nodes.childOf")} ${node.parent_id}`
                        : "-"}
                    </TableCell>
                    <TableCell>{(node.cores_available || []).join(", ")}</TableCell>
                    <TableCell>{node.usage_coefficient}</TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button size="sm" variant="flat" onPress={() => openEdit(node)}>
                          {t("common.edit")}
                        </Button>
                        <Button size="sm" variant="flat" onPress={() => handlePush(node)}>
                          {t("nodes.pushConfig")}
                        </Button>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteNodeId(node.id)}
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

      <ConfirmDialog
        title={t("nodes.deleteTitle")}
        isOpen={!!deleteNodeId}
        onClose={() => setDeleteNodeId(null)}
        onConfirm={handleDelete}
      >
        {t("nodes.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("nodes.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("nodes.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              isRequired
            />
            <Input
              label={t("nodes.hostname")}
              value={form.hostname}
              onChange={(e) => setForm({ ...form, hostname: e.target.value })}
            />
            <Input
              label={t("nodes.address")}
              value={form.address}
              onChange={(e) => setForm({ ...form, address: e.target.value })}
            />
            <Input
              type="number"
              label={t("nodes.usageCoefficient")}
              value={form.usage_coefficient.toString()}
              onChange={(e) =>
                setForm({ ...form, usage_coefficient: Number(e.target.value) })
              }
            />
            <Input
              label={t("nodes.parentId")}
              value={form.parent_id}
              onChange={(e) => setForm({ ...form, parent_id: e.target.value })}
              placeholder="UUID (optional)"
            />
            <Textarea
              label={t("nodes.labels")}
              value={form.labels}
              onChange={(e) => setForm({ ...form, labels: e.target.value })}
              className="font-mono"
            />
            <div className="space-y-2">
              <p className="text-sm font-medium">{t("nodes.groups")}</p>
              <div className="flex flex-wrap gap-2">
                {groups.map((g) => (
                  <Checkbox
                    key={g.id}
                    isSelected={form.selectedGroups.has(g.id)}
                    onValueChange={(selected) => {
                      const next = new Set(form.selectedGroups);
                      if (selected) next.add(g.id);
                      else next.delete(g.id);
                      setForm({ ...form, selectedGroups: next });
                    }}
                  >
                    {g.name}
                  </Checkbox>
                ))}
              </div>
            </div>
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

      <Modal isOpen={!!editNode} onClose={() => setEditNode(null)}>
        <ModalContent>
          <ModalHeader>{t("nodes.editTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("nodes.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
            <Input
              label={t("nodes.hostname")}
              value={form.hostname}
              onChange={(e) => setForm({ ...form, hostname: e.target.value })}
            />
            <Input
              label={t("nodes.address")}
              value={form.address}
              onChange={(e) => setForm({ ...form, address: e.target.value })}
            />
            <Input
              type="number"
              label={t("nodes.usageCoefficient")}
              value={form.usage_coefficient.toString()}
              onChange={(e) =>
                setForm({ ...form, usage_coefficient: Number(e.target.value) })
              }
            />
            <Input
              label={t("nodes.parentId")}
              value={form.parent_id}
              onChange={(e) => setForm({ ...form, parent_id: e.target.value })}
              placeholder="UUID (clear to remove)"
            />
            <Textarea
              label={t("nodes.labels")}
              value={form.labels}
              onChange={(e) => setForm({ ...form, labels: e.target.value })}
              className="font-mono"
            />
            <div className="space-y-2">
              <p className="text-sm font-medium">{t("nodes.groups")}</p>
              <div className="flex flex-wrap gap-2">
                {groups.map((g) => (
                  <Checkbox
                    key={g.id}
                    isSelected={form.selectedGroups.has(g.id)}
                    onValueChange={(selected) => {
                      const next = new Set(form.selectedGroups);
                      if (selected) next.add(g.id);
                      else next.delete(g.id);
                      setForm({ ...form, selectedGroups: next });
                    }}
                  >
                    {g.name}
                  </Checkbox>
                ))}
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setEditNode(null)}>
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
