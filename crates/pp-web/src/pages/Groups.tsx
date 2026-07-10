import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import { PageHeader, ConfirmDialog, Pagination, FormInput, FormTextArea } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getGroupsPaginated, createGroup, updateGroup, deleteGroup } from "../api/groups";
import { Group } from "../api/types";

interface GroupForm {
  name: string;
  description: string;
  labels: string;
}

export function Groups() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [groups, setGroups] = useState<Group[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editGroup, setEditGroup] = useState<Group | null>(null);
  const [deleteGroupId, setDeleteGroupId] = useState<string | null>(null);
  const [form, setForm] = useState<GroupForm>({
    name: "",
    description: "",
    labels: "{}",
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getGroupsPaginated(page, perPage);
      setGroups(res.data);
      setTotal(res.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  const resetForm = (group?: Group) => {
    if (group) {
      setForm({
        name: group.name,
        description: group.description || "",
        labels: JSON.stringify(group.labels || {}),
      });
    } else {
      setForm({
        name: "",
        description: "",
        labels: "{}",
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

  const handleCreate = async () => {
    try {
      await createGroup({
        name: form.name,
        description: form.description || undefined,
        labels: parseLabels(),
      });
      setCreateOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleUpdate = async () => {
    if (!editGroup) return;
    try {
      await updateGroup(editGroup.id, {
        name: form.name,
        description: form.description || undefined,
        labels: parseLabels(),
      });
      setEditGroup(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteGroupId) return;
    try {
      await deleteGroup(deleteGroupId);
      setDeleteGroupId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const openEdit = (group: Group) => {
    resetForm(group);
    setEditGroup(group);
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
          {loading ? (
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
