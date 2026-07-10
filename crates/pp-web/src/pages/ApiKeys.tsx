import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Badge, Modal, Spinner, Table } from "@heroui/react";
import {
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

export function ApiKeys() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } =
    usePagination();
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    scopes: '["*"]',
    ip_allowlist: "[]",
    rate_limit: "",
  });

  const fetch = async () => {
    setLoading(true);
    try {
      const res = await getApiKeys(page, perPage);
      setApiKeys(res.data);
      setTotal(res.pagination.total);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetch();
  }, [page, perPage]);

  const resetForm = () => {
    setForm({ name: "", scopes: '["*"]', ip_allowlist: "[]", rate_limit: "" });
  };

  const handleCreate = async () => {
    try {
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
      const res = await createApiKey({
        name: form.name,
        scopes,
        ip_allowlist: ipAllowlist,
        rate_limit: rateLimit,
      });
      const keyWithToken = res as ApiKeyWithToken;
      setNewToken(keyWithToken.token || null);
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
      await deleteApiKey(deleteId);
      setDeleteId(null);
      fetch();
    } catch {
      // error handled by axios interceptor
    }
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

      {newToken && (
        <CopyableSecret secret={newToken} label={t("apiKeys.tokenWarning")} />
      )}

      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table>
                <Table.ScrollContainer>
                  <Table.Content aria-label="api keys">
                    <Table.Header>
                      <Table.Column isRowHeader>
                        {t("common.name")}
                      </Table.Column>
                      <Table.Column>{t("apiKeys.scopes")}</Table.Column>
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
                      {apiKeys.map((key) => (
                        <Table.Row key={key.id}>
                          <Table.Cell>{key.name}</Table.Cell>
                          <Table.Cell>
                            <div className="flex flex-wrap gap-1">
                              {key.scopes.slice(0, 3).map((scope) => (
                                <Badge key={scope} size="sm" variant="soft">
                                  {scope}
                                </Badge>
                              ))}
                              {key.scopes.length > 3 && (
                                <Badge size="sm" variant="soft">
                                  +{key.scopes.length - 3}
                                </Badge>
                              )}
                            </div>
                          </Table.Cell>
                          <Table.Cell>
                            {key.is_active
                              ? t("common.enabled")
                              : t("common.disabled")}
                          </Table.Cell>
                          <Table.Cell>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteId(key.id)}
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
        title={t("apiKeys.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("apiKeys.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop
        isOpen={createOpen}
        onOpenChange={(open) => setCreateOpen(open)}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("apiKeys.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
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
            </Modal.Body>
            <Modal.Footer>
              <Button
                slot="close"
                variant="ghost"
                onPress={() => setCreateOpen(false)}
              >
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
