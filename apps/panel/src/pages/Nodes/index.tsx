import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { PageHeader, ConfirmDialog, Pagination } from "../../components/ui";
import { Button } from "../../components/ui/button";
import { Spinner } from "../../components/ui/spinner";
import { NodesTable } from "./NodesTable";
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
import { CertificateFormDialog } from "../CertificateFormDialog";

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
  const [certNode, setCertNode] = useState<Node | null>(null);
  const [binaries, setBinaries] = useState<CoreBinary[]>([]);
  const [binError, setBinError] = useState("");
  const [binLoading, setBinLoading] = useState(false);
  const [deleteBinary, setDeleteBinary] = useState<string | null>(null);
  const [pushResult, setPushResult] = useState<string | null>(null);
  const [pushPushing, setPushPushing] = useState(false);
  const [installCmd, setInstallCmd] = useState<InstallCommand | null>(null);
  const [rotateNode, setRotateNode] = useState<Node | null>(null);
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
    setBinError("");
    setBinLoading(true);
    try {
      const report = await getCoreBinaries(node.id);
      setBinaries(report.binaries);
      setBinError(report.error);
    } catch {
      setBinaries([]);
      setBinError(t("nodes.binariesReadError"));
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

  const requestInstallCommand = async (node: Node, rotate: boolean) => {
    setInstallLoading(true);
    try {
      const cmd = await getInstallCommand(node.id, rotate);
      setInstallCmd(cmd);
    } catch {
      // error handled by axios interceptor
    } finally {
      setInstallLoading(false);
    }
  };

  const openInstallCommand = (node: Node) => {
    if (node.last_seen_at) {
      setRotateNode(node);
    } else {
      requestInstallCommand(node, false);
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

      <NodesTable
        nodes={filteredNodes}
        pending={pending}
        isLoading={isLoading}
        onEdit={openEdit}
        onLogs={openLogs}
        onPush={openPush}
        onInstall={openInstallCommand}
        onBinaries={openBinaries}
        onCertificate={setCertNode}
        onDelete={(node) => setDeleteNodeId(node.id)}
      />

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

      <CertificateFormDialog
        key={certNode?.id}
        isOpen={!!certNode}
        nodes={nodes}
        initialNodeId={certNode?.id}
        onOpenChange={(open) => !open && setCertNode(null)}
      />

      <BinariesModal
        isOpen={!!binNode}
        node={binNode}
        binaries={binaries}
        binLoading={binLoading}
        binError={binError}
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

      <ConfirmDialog
        title={t("nodes.rotateCredentials")}
        confirmText={t("nodes.rotateCredentials")}
        isOpen={!!rotateNode}
        onClose={() => setRotateNode(null)}
        onConfirm={() => {
          if (rotateNode) requestInstallCommand(rotateNode, true);
          setRotateNode(null);
        }}
      >
        {t("nodes.rotateCredentialsConfirm", { name: rotateNode?.name })}
      </ConfirmDialog>

      <InstallModal
        isOpen={!!installCmd || installLoading}
        installCmd={installCmd}
        installLoading={installLoading}
        onClose={() => setInstallCmd(null)}
      />
    </div>
  );
}
