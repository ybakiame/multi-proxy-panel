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
  Spinner,
} from "@heroui/react";
import { PageHeader, ConfirmDialog, Pagination } from "../components/ui";
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
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table removeWrapper aria-label="groups">
              <TableHeader>
                <TableColumn>{t("common.name")}</TableColumn>
                <TableColumn>{t("groups.description")}</TableColumn>
                <TableColumn>{t("common.labels")}</TableColumn>
                <TableColumn>{t("common.actions")}</TableColumn>
              </TableHeader>
              <TableBody emptyContent={t("common.empty")}>
                {groups.map((group) => (
                  <TableRow key={group.id}>
                    <TableCell>{group.name}</TableCell>
                    <TableCell>{group.description || "-"}</TableCell>
                    <TableCell className="max-w-xs truncate">
                      {JSON.stringify(group.labels || {})}
                    </TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button size="sm" variant="flat" onPress={() => openEdit(group)}>
                          {t("common.edit")}
                        </Button>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteGroupId(group.id)}
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

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("groups.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              isRequired
            />
            <Input
              label={t("groups.description")}
              value={form.description}
              onChange={(e) => setForm({ ...form, description: e.target.value })}
            />
            <Textarea
              label={t("groups.labels")}
              value={form.labels}
              onChange={(e) => setForm({ ...form, labels: e.target.value })}
              className="font-mono"
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

      <Modal isOpen={!!editGroup} onClose={() => setEditGroup(null)}>
        <ModalContent>
          <ModalHeader>{t("groups.editTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
            <Input
              label={t("groups.description")}
              value={form.description}
              onChange={(e) => setForm({ ...form, description: e.target.value })}
            />
            <Textarea
              label={t("groups.labels")}
              value={form.labels}
              onChange={(e) => setForm({ ...form, labels: e.target.value })}
              className="font-mono"
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setEditGroup(null)}>
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
