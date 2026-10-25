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
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import {
  DataTable,
  PageHeader,
  ConfirmDialog,
  CopyableSecret,
  Pagination,
  FormInput,
  FormTextArea,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getApiKeys, createApiKey, deleteApiKey } from "../api/apiKeys";
import { ApiKey } from "../api/types";

interface ApiKeyWithToken extends ApiKey {
  token?: string;
}

const apiKeysQueryKey = "api-keys";

export function ApiKeys() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const { page, perPage, setPage, setPerPage } = usePagination();
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    scopes: '["*"]',
    ip_allowlist: "[]",
    rate_limit: "",
  });

  const { data: apiKeysData, isLoading: loading } = useQuery({
    queryKey: [apiKeysQueryKey, { page, perPage }],
    queryFn: () => getApiKeys(page, perPage),
  });

  const apiKeys = apiKeysData?.data ?? [];
  const total = apiKeysData?.pagination.total ?? 0;
  const totalPages = Math.max(1, Math.ceil(total / perPage));

  const createMutation = useMutation({
    mutationFn: createApiKey,
    onSuccess: (res) => {
      const keyWithToken = res as ApiKeyWithToken;
      setNewToken(keyWithToken.token || null);
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [apiKeysQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteApiKey,
    onSuccess: () => {
      setDeleteId(null);
      queryClient.invalidateQueries({ queryKey: [apiKeysQueryKey] });
    },
  });

  const resetForm = () => {
    setForm({ name: "", scopes: '["*"]', ip_allowlist: "[]", rate_limit: "" });
  };

  const handleCreate = () => {
    let scopes: string[] = ["*"];
    let ipAllowlist: string[] = [];
    let rateLimit: number | undefined;
    try {
      scopes = JSON.parse(form.scopes);
    } catch {}
    try {
      ipAllowlist = JSON.parse(form.ip_allowlist);
    } catch {}
    if (form.rate_limit) {
      rateLimit = Number(form.rate_limit);
    }
    createMutation.mutate({
      name: form.name,
      scopes,
      ip_allowlist: ipAllowlist,
      rate_limit: rateLimit,
    });
  };

  const handleDelete = () => {
    if (!deleteId) return;
    deleteMutation.mutate(deleteId);
  };

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("apiKeys.title")}
        action={{
          label: t("apiKeys.create"),
          onClick: () => {
            setNewToken(null);
            resetForm();
            setCreateOpen(true);
          },
        }}
      />

      {newToken && <CopyableSecret secret={newToken} label={t("apiKeys.tokenWarning")} />}

      <Card>
        <CardContent>
          <DataTable
            aria-label="api keys"
            columns={4}
            loading={loading}
            empty={apiKeys.length === 0 ? t("common.empty") : undefined}
            header={
              <TableRow>
                <TableHead>{t("common.name")}</TableHead>
                <TableHead>{t("apiKeys.scopes")}</TableHead>
                <TableHead>{t("common.active")}</TableHead>
                <TableHead>{t("common.actions")}</TableHead>
              </TableRow>
            }
          >
            {apiKeys.map((key) => (
              <TableRow key={key.id}>
                <TableCell>{key.name}</TableCell>
                <TableCell>
                  <div className="flex flex-wrap gap-1">
                    {key.scopes.slice(0, 3).map((scope) => (
                      <Badge key={scope} variant="secondary">
                        {scope}
                      </Badge>
                    ))}
                    {key.scopes.length > 3 && (
                      <Badge variant="secondary">+{key.scopes.length - 3}</Badge>
                    )}
                  </div>
                </TableCell>
                <TableCell>{key.is_active ? t("common.enabled") : t("common.disabled")}</TableCell>
                <TableCell>
                  <Button size="sm" variant="destructive" onClick={() => setDeleteId(key.id)}>
                    {t("common.delete")}
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </DataTable>
          <Pagination
            page={page}
            totalPages={totalPages}
            perPage={perPage}
            total={total}
            onPageChange={setPage}
            onPerPageChange={setPerPage}
          />
        </CardContent>
      </Card>

      <ConfirmDialog
        title={t("apiKeys.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("apiKeys.deleteConfirm")}
      </ConfirmDialog>

      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>{t("apiKeys.createTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <FormInput
              label={t("common.name")}
              value={form.name}
              onChange={(value) => setForm({ ...form, name: value })}
              isRequired
            />
            <FormTextArea
              label={t("apiKeys.scopes")}
              value={form.scopes}
              onChange={(value) => setForm({ ...form, scopes: value })}
              className="font-mono"
            />
            <FormTextArea
              label={t("apiKeys.ipAllowlist")}
              value={form.ip_allowlist}
              onChange={(value) => setForm({ ...form, ip_allowlist: value })}
              className="font-mono"
            />
            <FormInput
              type="number"
              label={t("apiKeys.rateLimit")}
              value={form.rate_limit}
              onChange={(value) => setForm({ ...form, rate_limit: value })}
            />
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
