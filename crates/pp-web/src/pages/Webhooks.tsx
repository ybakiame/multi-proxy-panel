import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Checkbox,
  Chip,
  Input,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
  Spinner,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  Textarea,
} from "@heroui/react";
import { PageHeader, ConfirmDialog, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getWebhooks, createWebhook, deleteWebhook } from "../api/webhooks";
import { Webhook } from "../api/types";

export function Webhooks() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [webhooks, setWebhooks] = useState<Webhook[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    url: "",
    events: '["client.created", "client.exceeded"]',
    secret: "",
    is_active: true,
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getWebhooks(page, perPage);
      setWebhooks(res.data);
      setTotal(res.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  const resetForm = () => {
    setForm({
      name: "",
      url: "",
      events: '["client.created", "client.exceeded"]',
      secret: "",
      is_active: true,
    });
  };

  const handleCreate = async () => {
    try {
      let events: string[] = [];
      try {
        events = JSON.parse(form.events);
      } catch {}
      await createWebhook({
        name: form.name,
        url: form.url,
        events,
        secret: form.secret || undefined,
        is_active: form.is_active,
      });
      setCreateOpen(false);
      resetForm();
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteId) return;
    try {
      await deleteWebhook(deleteId);
      setDeleteId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("webhooks.title")}
        action={{
          label: t("webhooks.create"),
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
            <>
              <Table removeWrapper aria-label="webhooks">
                <TableHeader>
                  <TableColumn>{t("common.name")}</TableColumn>
                  <TableColumn>{t("webhooks.url")}</TableColumn>
                  <TableColumn>{t("webhooks.events")}</TableColumn>
                  <TableColumn>{t("common.active")}</TableColumn>
                  <TableColumn>{t("common.actions")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("common.empty")}>
                  {webhooks.map((webhook) => (
                    <TableRow key={webhook.id}>
                      <TableCell>{webhook.name}</TableCell>
                      <TableCell className="max-w-xs truncate">{webhook.url}</TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {webhook.events.slice(0, 2).map((event) => (
                            <Chip key={event} size="sm" variant="flat">
                              {event}
                            </Chip>
                          ))}
                          {webhook.events.length > 2 && (
                            <Chip size="sm" variant="flat">+{webhook.events.length - 2}</Chip>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>{webhook.is_active ? t("common.enabled") : t("common.disabled")}</TableCell>
                      <TableCell>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteId(webhook.id)}
                        >
                          {t("common.delete")}
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
              <Pagination
                page={page}
                totalPages={totalPages}
                perPage={perPage}
                total={total}
                onPageChange={setPage}
                onPerPageChange={setPerPage}
              />
            </>
          )}
        </CardBody>
      </Card>

      <ConfirmDialog
        title={t("webhooks.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("webhooks.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("webhooks.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              isRequired
            />
            <Input
              label={t("webhooks.url")}
              value={form.url}
              onChange={(e) => setForm({ ...form, url: e.target.value })}
              isRequired
            />
            <Textarea
              label={t("webhooks.events")}
              value={form.events}
              onChange={(e) => setForm({ ...form, events: e.target.value })}
              className="font-mono"
            />
            <Input
              type="password"
              label={t("webhooks.secret")}
              value={form.secret}
              onChange={(e) => setForm({ ...form, secret: e.target.value })}
            />
            <Checkbox
              isSelected={form.is_active}
              onValueChange={(selected) => setForm({ ...form, is_active: selected })}
            >
              {t("common.active")}
            </Checkbox>
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
    </div>
  );
}
