import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { PageHeader, ConfirmDialog, StatusBadge, Pagination } from "../../components/ui";
import { Button } from "../../components/ui/button";
import { Card, CardContent } from "../../components/ui/card";
import { Spinner } from "../../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../../components/ui/table";
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
  const { page, perPage, setPage, setPerPage } = usePagination();

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
          <Button size="sm" onClick={handlePushAll} disabled={pushPushing}>
            {pushPushing ? <Spinner /> : t("nodes.pushAllPending")}
          </Button>
        </div>
      )}

      {pushResult && (
        <div className="rounded-lg bg-muted px-4 py-2 text-sm text-muted-foreground">
          {pushResult}
        </div>
      )}

      <div className="flex flex-wrap gap-2">
        {statusFilters.map((f) => (
          <Button
            key={f.key}
            size="sm"
            variant={statusFilter === f.key ? "default" : "ghost"}
            onClick={() => {
              setStatusFilter(f.key);
              setPage(1);
            }}
          >
            {f.label}
          </Button>
        ))}
      </div>

      <Card>
        <CardContent>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="nodes">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("nodes.name")}</TableHead>
                  <TableHead>{t("nodes.hostname")}</TableHead>
                  <TableHead>{t("nodes.address")}</TableHead>
                  <TableHead>{t("common.status")}</TableHead>
                  <TableHead>{t("nodes.pending")}</TableHead>
                  <TableHead>{t("nodes.coreStatus")}</TableHead>
                  <TableHead>{t("nodes.parentId")}</TableHead>
                  <TableHead>{t("nodes.coresAvailable")}</TableHead>
                  <TableHead>{t("nodes.usageCoefficient")}</TableHead>
                  <TableHead>{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {filteredNodes.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={10} className="p-4 text-center text-muted-foreground">
                      {t("common.empty")}
                    </TableCell>
                  </TableRow>
                ) : (
                  filteredNodes.map((node) => (
                    <TableRow key={node.id}>
                      <TableCell>{node.name}</TableCell>
                      <TableCell>{node.hostname || "-"}</TableCell>
                      <TableCell>{node.address || "-"}</TableCell>
                      <TableCell>
                        <StatusBadge status={node.status} />
                      </TableCell>
                      <TableCell>
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
                      </TableCell>
                      <TableCell>
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
                                {shortVersion && <span className="opacity-70">{shortVersion}</span>}
                                <span>
                                  {cs.running ? t("common.running") : t("common.stopped")}
                                </span>
                              </span>
                            );
                          })}
                          {(node.core_statuses || []).length === 0 && "-"}
                        </div>
                      </TableCell>
                      <TableCell>
                        {node.parent_id ? `${t("nodes.childOf")} ${node.parent_id}` : "-"}
                      </TableCell>
                      <TableCell>{(node.cores_available || []).join(", ")}</TableCell>
                      <TableCell>{node.usage_coefficient}</TableCell>
                      <TableCell>
                        <div className="flex gap-2">
                          <Button size="sm" variant="ghost" onClick={() => openEdit(node)}>
                            {t("common.edit")}
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => openLogs(node)}>
                            {t("nodes.logs")}
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => openPush(node)}>
                            {t("nodes.pushConfig")}
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => openInstallCommand(node)}
                          >
                            {t("nodes.installCommand")}
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => openBinaries(node)}>
                            {t("nodes.binaries")}
                          </Button>
                          <Button
                            size="sm"
                            variant="destructive"
                            onClick={() => setDeleteNodeId(node.id)}
                          >
                            {t("common.delete")}
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Pagination
        page={page}
        totalPages={totalPages}
        perPage={perPage}
        total={nodesData?.pagination.total ?? 0}
        onPageChange={setPage}
        onPerPageChange={setPerPage}
      />

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
