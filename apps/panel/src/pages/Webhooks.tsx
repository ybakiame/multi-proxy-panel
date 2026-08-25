import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Badge, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  Pagination,
  FormInput,
  FormTextArea,
  FormCheckbox,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getWebhooks, createWebhook, deleteWebhook } from "../api/webhooks";

const webhooksQueryKey = "webhooks";

export function Webhooks() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    url: "",
    events: '["client.created", "client.exceeded"]',
    secret: "",
    is_active: true,
  });

  const { data, isLoading } = useQuery({
    queryKey: [webhooksQueryKey, { page, perPage }],
    queryFn: () => getWebhooks(page, perPage),
  });

  const webhooks = data?.data ?? [];
  const total = data?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const createMutation = useMutation({
    mutationFn: createWebhook,
    onSuccess: () => {
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [webhooksQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteWebhook,
    onSuccess: () => {
      setDeleteId(null);
      queryClient.invalidateQueries({ queryKey: [webhooksQueryKey] });
    },
  });

  const resetForm = () => {
    setForm({
      name: "",
      url: "",
      events: '["client.created", "client.exceeded"]',
      secret: "",
      is_active: true,
    });
  };

  const handleCreate = () => {
    let events: string[] = [];
    try {
      events = JSON.parse(form.events);
    } catch {
      // ignore parse error
    }
    createMutation.mutate({
      name: form.name,
      url: form.url,
      events,
      secret: form.secret || undefined,
      is_active: form.is_active,
    });
  };

  const handleDelete = () => {
    if (!deleteId) return;
    deleteMutation.mutate(deleteId);
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
        <Card.Content>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="webhooks">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>{t("common.name")}</Table.Column>
                      <Table.Column>{t("webhooks.url")}</Table.Column>
                      <Table.Column>{t("webhooks.events")}</Table.Column>
                      <Table.Column>{t("common.active")}</Table.Column>
                      <Table.Column>{t("common.actions")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("common.empty")}
                        </div>
                      )}
                    >
                      {webhooks.map((webhook) => (
                        <Table.Row key={webhook.id}>
                          <Table.Cell>{webhook.name}</Table.Cell>
                          <Table.Cell className="max-w-xs truncate">{webhook.url}</Table.Cell>
                          <Table.Cell>
                            <div className="flex flex-wrap gap-1">
                              {webhook.events.slice(0, 2).map((event) => (
                                <Badge key={event} size="sm" variant="soft">
                                  {event}
                                </Badge>
                              ))}
                              {webhook.events.length > 2 && (
                                <Badge size="sm" variant="soft">
                                  +{webhook.events.length - 2}
                                </Badge>
                              )}
                            </div>
                          </Table.Cell>
                          <Table.Cell>
                            {webhook.is_active ? t("common.enabled") : t("common.disabled")}
                          </Table.Cell>
                          <Table.Cell>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteId(webhook.id)}
                            >
                              {t("common.delete")}
                            </Button>
                          </Table.Cell>
                        </Table.Row>
                      ))}
                    </Table.Body>
                  </Table.Content>
                </Table.ScrollContainer>
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
        </Card.Content>
      </Card>

      <ConfirmDialog
        title={t("webhooks.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("webhooks.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("webhooks.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormInput
                label={t("webhooks.url")}
                value={form.url}
                onChange={(value) => setForm({ ...form, url: value })}
                isRequired
              />
              <FormTextArea
                label={t("webhooks.events")}
                value={form.events}
                onChange={(value) => setForm({ ...form, events: value })}
                className="font-mono"
              />
              <FormInput
                type="password"
                label={t("webhooks.secret")}
                value={form.secret}
                onChange={(value) => setForm({ ...form, secret: value })}
              />
              <FormCheckbox
                isSelected={form.is_active}
                onChange={(selected) => setForm({ ...form, is_active: selected })}
              >
                {t("common.active")}
              </FormCheckbox>
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
    </div>
  );
}
