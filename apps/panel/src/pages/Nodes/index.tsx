import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { Button, Card, Spinner, Table } from "@heroui/react";
import { PageHeader, ConfirmDialog, StatusBadge } from "../../components/ui";
import { usePagination } from "../../hooks/useCommon";
import {
  getNodesPaginated,
  getNode,
  getNodeLogs,
  getCoreBinaries,
  getPendingUpdates,
  getInstallCommand,
  type CoreBinary,
  type InstallCommand,
} from "../../api/nodes";
import type { Node, AgentLog } from "../../api/types";
import { useNodeActions, defaultFormState, type NodeFormState } from "./useNodeActions";
import { InstallWizard } from "./InstallWizard";
import { EditModal, PushModal, BinariesModal, LogsModal, InstallModal } from "./NodeModals";

const nodesQueryKey = "nodes";
const pendingQueryKey = "pending-updates";

type StatusFilter = "all" | "connecting" | "online" | "offline";

export function Nodes() {
  const { t } = useTranslation();
  const { page, perPage, setPage } = usePagination();

  const [createOpen, setCreateOpen] = useState(false);
  const [wizardStep, setWizardStep] = useState<1 | 2>(1);
  const [newNodeId, setNewNodeId] = useState<string | null>(null);
  const [editNode, setEditNode] = useState<Node | null>(null);
  const [deleteNodeId, setDeleteNodeId] = useState<string | null>(null);
  const [logNode, setLogNode] = useState<Node | null>(null);
  const [nodeLogs, setNodeLogs] = useState<AgentLog[]>([]);
  const [logsLoading, setLogsLoading] = useState(false);
  const [pushNode, setPushNode] = useState<Node | null>(null);
  const [pushCore, setPushCore] = useState("sing-box");
  const [pushing, setPushing] = useState(false);
  const [binNode, setBinNode] = useState<Node | null>(null);
  const [binaries, setBinaries] = useState<CoreBinary[]>([]);
  const [binLoading, setBinLoading] = useState(false);
  const [deleteBinary, setDeleteBinary] = useState<string | null>(null);
  const [pushResult, setPushResult] = useState<string | null>(null);
  const [pushPushing, setPushPushing] = useState(false);
  const [installCmd, setInstallCmd] = useState<InstallCommand | null>(null);
  const [installLoading, setInstallLoading] = useState(false);
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("all");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [form, setForm] = useState<NodeFormState>(defaultFormState);

  const {
    createMutation,
    updateMutation,
    deleteMutation,
    pushMutation,
    pushAllMutation,
    deleteBinaryMutation,
    buildCreatePayload,
    buildUpdatePayload,
  } = useNodeActions();

  const { data: nodesData, isLoading } = useQuery({
    queryKey: [nodesQueryKey, { page, perPage }],
    queryFn: () => getNodesPaginated(page, perPage),
  });

  const nodes = nodesData?.data ?? [];
  const totalPages = nodesData?.pagination.total_pages ?? 1;

  const { data: pending = [] } = useQuery({
    queryKey: [pendingQueryKey],
    queryFn: getPendingUpdates,
  });

  // Poll new node status during wizard step 2
  const { data: pollingNode } = useQuery({
    queryKey: [nodesQueryKey, newNodeId],
    queryFn: () => (newNodeId ? getNode(newNodeId) : null),
    refetchInterval: 3000,
    enabled: !!newNodeId && wizardStep === 2,
  });

  const resetForm = (node?: Node) => {
    if (node) {
      setForm({
        name: node.name,
        domain: node.domain || "",
        usage_coefficient: node.usage_coefficient,
        labels: JSON.stringify(node.labels || {}),
        parent_id: node.parent_id || "",
      });
    } else {
      setForm(defaultFormState);
    }
  };

  const handleCreate = () => {
    const payload = buildCreatePayload(form);
    createMutation.mutate(payload, {
      onSuccess: (res) => {
        setNewNodeId(res.id);
        setWizardStep(2);
        resetForm();
        if (res.id) {
          setInstallLoading(true);
          getInstallCommand(res.id)
            .then((cmd) => setInstallCmd(cmd))
            .catch(() => {
              // error handled by axios interceptor
            })
            .finally(() => setInstallLoading(false));
        }
      },
    });
  };

  const handleUpdate = () => {
    if (!editNode) return;
    const payload = buildUpdatePayload(form);
    updateMutation.mutate(
      { id: editNode.id, data: payload },
      {
        onSuccess: () => setEditNode(null),
      },
    );
  };

  const handleDelete = () => {
    if (!deleteNodeId) return;
    deleteMutation.mutate(deleteNodeId, {
      onSuccess: () => setDeleteNodeId(null),
    });
  };

  const openPush = (node: Node) => {
    const cores = (node.cores_available || []).filter(Boolean);
    setPushCore(cores[0] || "sing-box");
    setPushNode(node);
  };

  const handlePush = () => {
    if (!pushNode || !pushCore) return;
    setPushing(true);
    pushMutation.mutate(
      { id: pushNode.id, config: { core_type: pushCore, restart: true } },
      {
        onSettled: () => {
          setPushing(false);
          setPushNode(null);
        },
      },
    );
  };

  const openBinaries = async (node: Node) => {
    setBinNode(node);
    setBinLoading(true);
    try {
      setBinaries(await getCoreBinaries(node.id));
    } catch {
      // error handled by axios interceptor
    } finally {
      setBinLoading(false);
    }
  };

  const handleDeleteBinary = () => {
    if (!binNode || !deleteBinary) return;
    deleteBinaryMutation.mutate(
      { nodeId: binNode.id, fileName: deleteBinary },
      {
        onSuccess: () => setDeleteBinary(null),
      },
    );
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

  const openInstallCommand = async (node: Node) => {
    setInstallLoading(true);
    try {
      const cmd = await getInstallCommand(node.id);
      setInstallCmd(cmd);
    } catch {
      // error handled by axios interceptor
    } finally {
      setInstallLoading(false);
    }
  };

  const closeWizard = () => {
    setCreateOpen(false);
    setWizardStep(1);
    setNewNodeId(null);
    setInstallCmd(null);
  };

  const handlePushAll = () => {
    setPushPushing(true);
    pushAllMutation.mutate(
      {},
      {
        onSuccess: (r) => {
          setPushResult(t("nodes.pushResult", { ok: r.succeeded, fail: r.failed }));
        },
        onSettled: () => setPushPushing(false),
      },
    );
  };

  const filteredNodes =
    statusFilter === "all" ? nodes : nodes.filter((n) => n.status === statusFilter);

  const statusFilters: { key: StatusFilter; label: string }[] = [
    { key: "all", label: t("nodes.filterAll") },
    { key: "connecting", label: t("nodes.filterConnecting") },
    { key: "online", label: t("nodes.filterOnline") },
    { key: "offline", label: t("nodes.filterOffline") },
  ];

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("nodes.title")}
        action={{
          label: t("nodes.create"),
          onClick: () => {
            resetForm();
            setWizardStep(1);
            setNewNodeId(null);
            setInstallCmd(null);
            setCreateOpen(true);
          },
        }}
      />

      {pending.length > 0 && (
        <div className="flex items-center gap-4 rounded-lg bg-warning-soft px-4 py-3 text-sm text-warning-soft-foreground">
          <span className="flex-1">{t("nodes.pendingSummary", { count: pending.length })}</span>
          <Button size="sm" variant="primary" onPress={handlePushAll} isDisabled={pushPushing}>
            {pushPushing ? <Spinner size="sm" /> : t("nodes.pushAllPending")}
          </Button>
        </div>
      )}

      {pushResult && (
        <div className="rounded-lg bg-default-soft px-4 py-2 text-sm text-default-soft-foreground">
          {pushResult}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        {statusFilters.map((f) => (
          <Button
            key={f.key}
            size="sm"
            variant={statusFilter === f.key ? "primary" : "ghost"}
            onPress={() => {
              setStatusFilter(f.key);
              setPage(1);
            }}
          >
            {f.label}
          </Button>
        ))}
      </div>

      <Card>
        <Card.Content>
          {isLoading ? (
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
                    <Table.Column>{t("nodes.pending")}</Table.Column>
                    <Table.Column>{t("nodes.coreStatus")}</Table.Column>
                    <Table.Column>{t("nodes.parentId")}</Table.Column>
                    <Table.Column>{t("nodes.coresAvailable")}</Table.Column>
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
                    {filteredNodes.map((node) => (
                      <Table.Row key={node.id}>
                        <Table.Cell>{node.name}</Table.Cell>
                        <Table.Cell>{node.hostname || "-"}</Table.Cell>
                        <Table.Cell>{node.address || "-"}</Table.Cell>
                        <Table.Cell>
                          <StatusBadge status={node.status} />
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex flex-wrap gap-1">
                            {pending
                              .filter((p) => p.node_id === node.id)
                              .map((p, i) => (
                                <span
                                  key={i}
                                  className="inline-flex items-center whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium bg-warning-soft text-warning-soft-foreground"
                                >
                                  {p.update_type === "core"
                                    ? `${t("nodes.pendingCore")} ${p.core_type}`
                                    : `${t("nodes.pendingConfig")} ${p.core_type}`}
                                </span>
                              ))}
                            {pending.filter((p) => p.node_id === node.id).length === 0 && null}
                          </div>
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex flex-wrap gap-1">
                            {(node.core_statuses || []).map((cs) => {
                              const shortVersion = cs.version.match(/\d+\.\d+[\d.]*/)?.[0] ?? "";
                              return (
                                <span
                                  key={cs.core_type}
                                  className={`inline-flex items-center gap-1 whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium ${
                                    cs.running
                                      ? "bg-success-soft text-success-soft-foreground"
                                      : "bg-danger-soft text-danger-soft-foreground"
                                  }`}
                                  title={cs.version}
                                >
                                  {cs.core_type}
                                  {shortVersion && (
                                    <span className="opacity-70">{shortVersion}</span>
                                  )}
                                  <span>
                                    {cs.running ? t("common.running") : t("common.stopped")}
                                  </span>
                                </span>
                              );
                            })}
                            {(node.core_statuses || []).length === 0 && "-"}
                          </div>
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
                            <Button size="sm" variant="ghost" onPress={() => openPush(node)}>
                              {t("nodes.pushConfig")}
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              onPress={() => openInstallCommand(node)}
                            >
                              {t("nodes.installCommand")}
                            </Button>
                            <Button size="sm" variant="ghost" onPress={() => openBinaries(node)}>
                              {t("nodes.binaries")}
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

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-center gap-2 py-4">
          <Button
            isIconOnly
            variant="ghost"
            size="sm"
            isDisabled={page <= 1}
            onPress={() => setPage(page - 1)}
          >
            ‹
          </Button>
          <span className="text-sm text-muted-foreground">
            {t("pagination.pageInfo", {
              current: page,
              total: totalPages,
              count: nodesData?.pagination.total ?? 0,
            })}
          </span>
          <Button
            isIconOnly
            variant="ghost"
            size="sm"
            isDisabled={page >= totalPages}
            onPress={() => setPage(page + 1)}
          >
            ›
          </Button>
        </div>
      )}

      <ConfirmDialog
        title={t("nodes.deleteTitle")}
        isOpen={!!deleteNodeId}
        onClose={() => setDeleteNodeId(null)}
        onConfirm={handleDelete}
      >
        {t("nodes.deleteConfirm")}
      </ConfirmDialog>

      <InstallWizard
        isOpen={createOpen}
        wizardStep={wizardStep}
        newNodeId={newNodeId}
        form={form}
        showAdvanced={showAdvanced}
        installCmd={installCmd}
        installLoading={installLoading}
        createPending={createMutation.isPending}
        pollingNode={pollingNode ?? null}
        onClose={closeWizard}
        onChangeForm={setForm}
        onToggleAdvanced={() => setShowAdvanced(!showAdvanced)}
        onCreate={handleCreate}
      />

      <EditModal
        isOpen={!!editNode}
        node={editNode}
        form={form}
        onClose={() => setEditNode(null)}
        onChange={setForm}
        onConfirm={handleUpdate}
      />

      <PushModal
        isOpen={!!pushNode}
        node={pushNode}
        pushCore={pushCore}
        pushing={pushing}
        onClose={() => setPushNode(null)}
        onChangeCore={setPushCore}
        onConfirm={handlePush}
      />

      <BinariesModal
        isOpen={!!binNode}
        node={binNode}
        binaries={binaries}
        binLoading={binLoading}
        deleteBinary={deleteBinary}
        onClose={() => setBinNode(null)}
        onDelete={setDeleteBinary}
        onConfirmDelete={handleDeleteBinary}
        onCancelDelete={() => setDeleteBinary(null)}
      />

      <LogsModal
        isOpen={!!logNode}
        node={logNode}
        logs={nodeLogs}
        logsLoading={logsLoading}
        onClose={() => setLogNode(null)}
      />

      <InstallModal
        isOpen={!!installCmd || installLoading}
        installCmd={installCmd}
        installLoading={installLoading}
        onClose={() => setInstallCmd(null)}
      />
    </div>
  );
}
