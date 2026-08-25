import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  CopyableSecret,
  StatusBadge,
  FormInput,
  FormSelect,
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
  getCoreBinaries,
  deleteCoreBinary,
  getPendingUpdates,
  pushPendingUpdates,
  getInstallCommand,
  type CoreBinary,
  type InstallCommand,
  type UpdateNodePayload,
} from "../api/nodes";
import { getNode } from "../api/nodes";
import type { Node, AgentLog } from "../api/types";

const nodesQueryKey = "nodes";
const pendingQueryKey = "pending-updates";

type StatusFilter = "all" | "connecting" | "online" | "offline";

export function Nodes() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
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
  const [form, setForm] = useState({
    name: "",
    domain: "",
    usage_coefficient: 1,
    labels: "{}",
    parent_id: "",
  });

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

  const createMutation = useMutation({
    mutationFn: createNode,
    onSuccess: (res) => {
      setNewNodeId(res.id);
      setWizardStep(2);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
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

  const updateMutation = useMutation({
    mutationFn: (payload: { id: string; data: UpdateNodePayload }) =>
      updateNode(payload.id, payload.data),
    onSuccess: () => {
      setEditNode(null);
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteNode,
    onSuccess: () => {
      setDeleteNodeId(null);
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const pushMutation = useMutation({
    mutationFn: (payload: { id: string; config: Record<string, unknown> }) =>
      pushConfig(payload.id, payload.config),
    onSuccess: () => {
      setPushNode(null);
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const pushAllMutation = useMutation({
    mutationFn: pushPendingUpdates,
    onSuccess: (r) => {
      setPushResult(t("nodes.pushResult", { ok: r.succeeded, fail: r.failed }));
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey] });
      queryClient.invalidateQueries({ queryKey: [pendingQueryKey] });
    },
  });

  const deleteBinaryMutation = useMutation({
    mutationFn: (payload: { nodeId: string; fileName: string }) =>
      deleteCoreBinary(payload.nodeId, payload.fileName),
    onSuccess: (_, payload) => {
      setDeleteBinary(null);
      queryClient.invalidateQueries({ queryKey: [nodesQueryKey, payload.nodeId, "binaries"] });
      // Refresh local binaries list for the open modal
      getCoreBinaries(payload.nodeId)
        .then((res) => setBinaries(res))
        .catch(() => {
          // error handled by axios interceptor
        });
    },
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
      setForm({
        name: "",
        domain: "",
        usage_coefficient: 1,
        labels: "{}",
        parent_id: "",
      });
    }
  };

  const handleCreate = () => {
    let labels: Record<string, string> = {};
    try {
      labels = JSON.parse(form.labels);
    } catch {
      // ignore parse error
    }
    createMutation.mutate({
      name: form.name,
      domain: form.domain || undefined,
      usage_coefficient: form.usage_coefficient,
      labels,
      parent_id: form.parent_id || undefined,
    });
  };

  const handleUpdate = () => {
    if (!editNode) return;
    let labels: Record<string, string> = {};
    try {
      labels = JSON.parse(form.labels);
    } catch {
      // ignore parse error
    }
    const payload: UpdateNodePayload = {
      name: form.name,
      domain: form.domain || undefined,
      usage_coefficient: form.usage_coefficient,
      labels,
      parent_id: form.parent_id || undefined,
    };
    updateMutation.mutate({ id: editNode.id, data: payload });
  };

  const handleDelete = () => {
    if (!deleteNodeId) return;
    deleteMutation.mutate(deleteNodeId);
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
        onSettled: () => setPushing(false),
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
    deleteBinaryMutation.mutate({ nodeId: binNode.id, fileName: deleteBinary });
  };

  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    if (bytes >= 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${bytes} B`;
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
          <Button
            size="sm"
            variant="primary"
            onPress={() => {
              setPushPushing(true);
              pushAllMutation.mutate(
                {},
                {
                  onSettled: () => setPushPushing(false),
                },
              );
            }}
            isDisabled={pushPushing}
          >
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

      {/* Create Wizard Modal */}
      <Modal.Backdrop
        isOpen={createOpen}
        onOpenChange={(open) => {
          if (!open) closeWizard();
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("nodes.wizardTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              {wizardStep === 1 ? (
                <>
                  <FormInput
                    label={t("nodes.name")}
                    value={form.name}
                    onChange={(value) => setForm({ ...form, name: value })}
                    isRequired
                  />
                  <FormInput
                    label={t("nodes.domain")}
                    value={form.domain}
                    onChange={(value) => setForm({ ...form, domain: value })}
                    placeholder={t("common.optional")}
                  />
                  <p className="text-sm text-muted-foreground">{t("nodes.step1Hint")}</p>
                  <Button variant="ghost" size="sm" onPress={() => setShowAdvanced(!showAdvanced)}>
                    {showAdvanced ? t("common.collapse") : t("common.expand")}
                  </Button>
                  {showAdvanced && (
                    <div className="space-y-4">
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
                    </div>
                  )}
                </>
              ) : (
                <>
                  <h3 className="text-sm font-medium">{t("nodes.step2Title")}</h3>
                  {installLoading ? (
                    <div className="flex h-32 items-center justify-center">
                      <Spinner />
                    </div>
                  ) : installCmd ? (
                    <>
                      {installCmd.was_connected && (
                        <div className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-sm text-warning">
                          {t("nodes.installWarning")}
                        </div>
                      )}
                      <p className="text-sm text-muted-foreground">{t("nodes.installHint")}</p>
                      <CopyableSecret
                        secret={installCmd.command}
                        label={t("nodes.installCommand")}
                      />
                      <div className="space-y-1 text-sm text-muted-foreground">
                        <p>
                          <span className="font-medium">{t("nodes.scriptUrl")}:</span>{" "}
                          {installCmd.script_url}
                        </p>
                        <p>
                          <span className="font-medium">{t("nodes.hubUrl")}:</span>{" "}
                          {installCmd.hub_url}
                        </p>
                        <p>
                          <span className="font-medium">{t("nodes.version")}:</span>{" "}
                          {installCmd.version}
                        </p>
                      </div>
                      <div className="flex items-center gap-3 rounded-lg bg-default-soft p-3">
                        {pollingNode?.status === "online" ? (
                          <>
                            <span className="inline-flex h-6 w-6 items-center justify-center rounded-full bg-success text-success-foreground">
                              ✓
                            </span>
                            <span className="text-sm font-medium">{t("nodes.connected")}</span>
                            <Link to="/bindings">
                              <Button size="sm" variant="primary">
                                {t("nodes.gotoBindings")}
                              </Button>
                            </Link>
                          </>
                        ) : (
                          <>
                            <Spinner size="sm" />
                            <span className="text-sm text-muted-foreground">
                              {t("nodes.waitingConnect")}
                            </span>
                          </>
                        )}
                      </div>
                    </>
                  ) : null}
                </>
              )}
            </Modal.Body>
            <Modal.Footer>
              {wizardStep === 1 ? (
                <>
                  <Button slot="close" variant="ghost" onPress={closeWizard}>
                    {t("common.cancel")}
                  </Button>
                  <Button
                    onPress={handleCreate}
                    isDisabled={!form.name.trim() || createMutation.isPending}
                  >
                    {createMutation.isPending ? <Spinner size="sm" /> : t("common.create")}
                  </Button>
                </>
              ) : (
                <Button slot="close" variant="ghost" onPress={closeWizard}>
                  {t("common.close")}
                </Button>
              )}
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* Edit Modal */}
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
                label={t("nodes.domain")}
                value={form.domain}
                onChange={(value) => setForm({ ...form, domain: value })}
                placeholder={t("common.optional")}
              />
              <FormInput label={t("nodes.hostname")} value={editNode?.hostname || ""} isReadOnly />
              <FormInput label={t("nodes.address")} value={editNode?.address || ""} isReadOnly />
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
        isOpen={!!pushNode}
        onOpenChange={(open) => {
          if (!open) setPushNode(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {pushNode ? `${t("nodes.pushTitle")}: ${pushNode.name}` : t("nodes.pushTitle")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("nodes.selectCore")}
                value={pushCore}
                onChange={setPushCore}
                options={((pushNode?.cores_available || []).filter(Boolean).length > 0
                  ? (pushNode?.cores_available || []).filter(Boolean)
                  : ["sing-box", "mihomo"]
                ).map((core) => ({ id: core, label: core }))}
                isRequired
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setPushNode(null)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handlePush} isDisabled={pushing || !pushCore}>
                {pushing ? <Spinner size="sm" /> : t("nodes.pushConfig")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <Modal.Backdrop
        isOpen={!!binNode}
        onOpenChange={(open) => {
          if (!open) setBinNode(null);
        }}
      >
        <Modal.Container className="max-w-2xl">
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {binNode
                  ? `${t("nodes.binariesTitle")}: ${binNode.name}`
                  : t("nodes.binariesTitle")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="max-h-[60vh] overflow-auto">
              {binLoading ? (
                <div className="flex h-32 items-center justify-center">
                  <Spinner />
                </div>
              ) : binaries.length === 0 ? (
                <div className="p-4 text-center text-muted-foreground">{t("common.empty")}</div>
              ) : (
                <Table aria-label="core binaries">
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("nodes.binaryName")}</Table.Column>
                      <Table.Column>{t("nodes.binarySize")}</Table.Column>
                      <Table.Column>{t("nodes.binaryModified")}</Table.Column>
                      <Table.Column>{t("common.status")}</Table.Column>
                      <Table.Column>{t("common.actions")}</Table.Column>
                    </Table.Header>
                    <Table.Body>
                      {binaries.map((b) => (
                        <Table.Row key={b.file_name}>
                          <Table.Cell className="font-mono text-sm">{b.file_name}</Table.Cell>
                          <Table.Cell>{formatSize(b.size_bytes)}</Table.Cell>
                          <Table.Cell>
                            {b.modified_at ? new Date(b.modified_at * 1000).toLocaleString() : "-"}
                          </Table.Cell>
                          <Table.Cell>
                            {b.in_use ? (
                              <span className="rounded bg-success-soft px-2 py-0.5 text-xs font-medium text-success-soft-foreground">
                                {t("nodes.binaryInUse")}
                              </span>
                            ) : (
                              "-"
                            )}
                          </Table.Cell>
                          <Table.Cell>
                            <Button
                              size="sm"
                              variant="danger"
                              isDisabled={b.in_use}
                              onPress={() => setDeleteBinary(b.file_name)}
                            >
                              {t("common.delete")}
                            </Button>
                          </Table.Cell>
                        </Table.Row>
                      ))}
                    </Table.Body>
                  </Table.Content>
                </Table>
              )}
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" onPress={() => setBinNode(null)}>
                {t("common.close")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <ConfirmDialog
        title={t("nodes.deleteBinaryTitle")}
        isOpen={!!deleteBinary}
        onClose={() => setDeleteBinary(null)}
        onConfirm={handleDeleteBinary}
      >
        {t("nodes.deleteBinaryConfirm", { file: deleteBinary })}
      </ConfirmDialog>

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
                  <div key={log.id} className="border-b border-separator pb-2 text-sm">
                    <div className="flex items-center gap-2">
                      <span
                        className={`rounded px-1.5 py-0.5 text-xs font-medium ${
                          log.level === "error"
                            ? "bg-danger-soft text-danger-soft-foreground"
                            : log.level === "warn"
                              ? "bg-warning-soft text-warning-soft-foreground"
                              : "bg-default-soft text-default-soft-foreground"
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

      <Modal.Backdrop
        isOpen={!!installCmd || installLoading}
        onOpenChange={(open) => {
          if (!open) setInstallCmd(null);
        }}
      >
        <Modal.Container className="max-w-3xl">
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {installCmd
                  ? `${t("nodes.installTitle")}: ${installCmd.name}`
                  : t("nodes.installTitle")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              {installLoading ? (
                <div className="flex h-32 items-center justify-center">
                  <Spinner />
                </div>
              ) : installCmd ? (
                <>
                  {installCmd.was_connected && (
                    <div className="rounded-lg border border-warning/30 bg-warning/10 p-3 text-sm text-warning">
                      {t("nodes.installWarning")}
                    </div>
                  )}
                  <p className="text-sm text-muted-foreground">{t("nodes.installHint")}</p>
                  <CopyableSecret secret={installCmd.command} label={t("nodes.installCommand")} />
                  <div className="space-y-1 text-sm text-muted-foreground">
                    <p>
                      <span className="font-medium">{t("nodes.scriptUrl")}:</span>{" "}
                      {installCmd.script_url}
                    </p>
                    <p>
                      <span className="font-medium">{t("nodes.hubUrl")}:</span> {installCmd.hub_url}
                    </p>
                    <p>
                      <span className="font-medium">{t("nodes.version")}:</span>{" "}
                      {installCmd.version}
                    </p>
                  </div>
                </>
              ) : null}
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setInstallCmd(null)}>
                {t("common.close")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
