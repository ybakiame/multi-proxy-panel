import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  Pagination,
  FormInput,
  FormTextArea,
  FormCheckbox,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getGroupsPaginated, createGroup, updateGroup, deleteGroup } from "../api/groups";
import { getBindings } from "../api/bindings";
import { getNodes } from "../api/nodes";
import { getAllProtocols } from "../api/protocols";
import { Group, Binding } from "../api/types";

const groupsQueryKey = "groups";
const bindingsQueryKey = "bindings";
const nodesQueryKey = "nodes";
const protocolsQueryKey = "protocols";

interface GroupForm {
  name: string;
  description: string;
  labels: string;
  selectedBindings: Set<string>;
}

export function Groups() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [editGroup, setEditGroup] = useState<Group | null>(null);
  const [deleteGroupId, setDeleteGroupId] = useState<string | null>(null);
  const [form, setForm] = useState<GroupForm>({
    name: "",
    description: "",
    labels: "{}",
    selectedBindings: new Set<string>(),
  });

  const { data: groupsData, isLoading } = useQuery({
    queryKey: [groupsQueryKey, { page, perPage }],
    queryFn: () => getGroupsPaginated(page, perPage),
  });

  const groups = groupsData?.data ?? [];
  const total = groupsData?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const { data: bindings = [] } = useQuery({
    queryKey: [bindingsQueryKey],
    queryFn: getBindings,
  });

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const { data: configs = [] } = useQuery({
    queryKey: [protocolsQueryKey],
    queryFn: getAllProtocols,
  });

  const createMutation = useMutation({
    mutationFn: (payload: {
      name: string;
      description?: string;
      labels?: Record<string, string>;
      binding_ids?: string[];
    }) => createGroup(payload),
    onSuccess: () => {
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [groupsQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: {
      id: string;
      data: {
        name: string;
        description?: string;
        labels?: Record<string, string>;
        binding_ids?: string[];
      };
    }) => updateGroup(payload.id, payload.data),
    onSuccess: () => {
      setEditGroup(null);
      queryClient.invalidateQueries({ queryKey: [groupsQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteGroup,
    onSuccess: () => {
      setDeleteGroupId(null);
      queryClient.invalidateQueries({ queryKey: [groupsQueryKey] });
    },
  });

  const resetForm = (group?: Group) => {
    if (group) {
      setForm({
        name: group.name,
        description: group.description || "",
        labels: JSON.stringify(group.labels || {}),
        selectedBindings: new Set(group.binding_ids || []),
      });
    } else {
      setForm({
        name: "",
        description: "",
        labels: "{}",
        selectedBindings: new Set<string>(),
      });
    }
  };

  const parseLabels = (): Record<string, string> => {
    try {
      return JSON.parse(form.labels);
    } catch {
      return {};
    }
  };

  const handleCreate = () => {
    createMutation.mutate({
      name: form.name,
      description: form.description || undefined,
      labels: parseLabels(),
      binding_ids: Array.from(form.selectedBindings),
    });
  };

  const handleUpdate = () => {
    if (!editGroup) return;
    updateMutation.mutate({
      id: editGroup.id,
      data: {
        name: form.name,
        description: form.description || undefined,
        labels: parseLabels(),
        binding_ids: Array.from(form.selectedBindings),
      },
    });
  };

  const handleDelete = () => {
    if (!deleteGroupId) return;
    deleteMutation.mutate(deleteGroupId);
  };

  const openEdit = (group: Group) => {
    resetForm(group);
    setEditGroup(group);
  };

  const bindingDisplayName = (binding: Binding) => {
    const node = nodes.find((n) => n.id === binding.node_id);
    const cfg = configs.find((c) => c.id === binding.protocol_config_id);
    return `${node?.name || binding.node_id} - ${cfg?.name || binding.protocol_config_id}`;
  };

  const bindingNamesForGroup = (groupId: string) => {
    return (
      bindings
        .filter((b) => b.group_ids?.includes(groupId))
        .map(bindingDisplayName)
        .join(", ") || "-"
    );
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("groups.title")}
        action={{
          label: t("groups.create"),
          onClick: () => {
            resetForm();
            setCreateOpen(true);
          },
        }}
      />

      <Card>
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="groups">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("common.name")}</Table.Column>
                    <Table.Column>{t("groups.description")}</Table.Column>
                    <Table.Column>{t("groups.bindings")}</Table.Column>
                    <Table.Column>{t("common.labels")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </div>
                    )}
                  >
                    {groups.map((group) => (
                      <Table.Row key={group.id}>
                        <Table.Cell>{group.name}</Table.Cell>
                        <Table.Cell>{group.description || "-"}</Table.Cell>
                        <Table.Cell className="max-w-xs truncate">
                          {bindingNamesForGroup(group.id)}
                        </Table.Cell>
                        <Table.Cell className="max-w-xs truncate">
                          {JSON.stringify(group.labels || {})}
                        </Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            <Button size="sm" variant="ghost" onPress={() => openEdit(group)}>
                              {t("common.edit")}
                            </Button>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteGroupId(group.id)}
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

      <Pagination
        page={page}
        totalPages={totalPages}
        perPage={perPage}
        total={total}
        onPageChange={setPage}
        onPerPageChange={setPerPage}
      />

      <ConfirmDialog
        title={t("groups.deleteTitle")}
        isOpen={!!deleteGroupId}
        onClose={() => setDeleteGroupId(null)}
        onConfirm={handleDelete}
      >
        {t("groups.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("groups.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormInput
                label={t("groups.description")}
                value={form.description}
                onChange={(value) => setForm({ ...form, description: value })}
              />
              <FormTextArea
                label={t("groups.labels")}
                value={form.labels}
                onChange={(value) => setForm({ ...form, labels: value })}
                className="font-mono"
              />
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("groups.bindings")}</p>
                <div className="flex flex-wrap gap-2">
                  {bindings.map((binding) => (
                    <FormCheckbox
                      key={binding.id}
                      isSelected={form.selectedBindings.has(binding.id)}
                      onChange={(selected) => {
                        const next = new Set(form.selectedBindings);
                        if (selected) next.add(binding.id);
                        else next.delete(binding.id);
                        setForm({ ...form, selectedBindings: next });
                      }}
                    >
                      {bindingDisplayName(binding)}
                    </FormCheckbox>
                  ))}
                </div>
              </div>
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
        isOpen={!!editGroup}
        onOpenChange={(open) => {
          if (!open) setEditGroup(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("groups.editTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
              />
              <FormInput
                label={t("groups.description")}
                value={form.description}
                onChange={(value) => setForm({ ...form, description: value })}
              />
              <FormTextArea
                label={t("groups.labels")}
                value={form.labels}
                onChange={(value) => setForm({ ...form, labels: value })}
                className="font-mono"
              />
              <div className="space-y-2">
                <p className="text-sm font-medium">{t("groups.bindings")}</p>
                <div className="flex flex-wrap gap-2">
                  {bindings.map((binding) => (
                    <FormCheckbox
                      key={binding.id}
                      isSelected={form.selectedBindings.has(binding.id)}
                      onChange={(selected) => {
                        const next = new Set(form.selectedBindings);
                        if (selected) next.add(binding.id);
                        else next.delete(binding.id);
                        setForm({ ...form, selectedBindings: next });
                      }}
                    >
                      {bindingDisplayName(binding)}
                    </FormCheckbox>
                  ))}
                </div>
              </div>
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setEditGroup(null)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleUpdate}>{t("common.update")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
