import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Badge } from "../components/ui/badge";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { Spinner } from "../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
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
        <CardContent>
          {isLoading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="webhooks">
                <TableHeader>
                  <TableRow>
                    <TableHead>{t("common.name")}</TableHead>
                    <TableHead>{t("webhooks.url")}</TableHead>
                    <TableHead>{t("webhooks.events")}</TableHead>
                    <TableHead>{t("common.active")}</TableHead>
                    <TableHead>{t("common.actions")}</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {webhooks.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={5} className="p-4 text-center text-muted-foreground">
                        {t("common.empty")}
                      </TableCell>
                    </TableRow>
                  )}
                  {webhooks.map((webhook) => (
                    <TableRow key={webhook.id}>
                      <TableCell>{webhook.name}</TableCell>
                      <TableCell className="max-w-xs truncate">{webhook.url}</TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {webhook.events.slice(0, 2).map((event) => (
                            <Badge key={event} variant="secondary">
                              {event}
                            </Badge>
                          ))}
                          {webhook.events.length > 2 && (
                            <Badge variant="secondary">+{webhook.events.length - 2}</Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>
                        {webhook.is_active ? t("common.enabled") : t("common.disabled")}
                      </TableCell>
                      <TableCell>
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => setDeleteId(webhook.id)}
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
        </CardContent>
      </Card>

      <ConfirmDialog
        title={t("webhooks.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("webhooks.deleteConfirm")}
      </ConfirmDialog>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("webhooks.createTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleCreate}>{t("common.create")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
