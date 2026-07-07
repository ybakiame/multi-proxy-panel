import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
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
import { PageHeader, ConfirmDialog, CopyableSecret, Pagination } from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import { getApiKeys, createApiKey, deleteApiKey } from "../api/apiKeys";
import { ApiKey } from "../api/types";

interface ApiKeyWithToken extends ApiKey {
  token?: string;
}

export function ApiKeys() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [apiKeys, setApiKeys] = useState<ApiKey[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    scopes: "[\"*\"]",
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
    setForm({ name: "", scopes: "[\"*\"]", ip_allowlist: "[]", rate_limit: "" });
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
        <CopyableSecret
          secret={newToken}
          label={t("apiKeys.tokenWarning")}
        />
      )}

      <Card>
        <CardBody>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table removeWrapper aria-label="api keys">
                <TableHeader>
                  <TableColumn>{t("common.name")}</TableColumn>
                  <TableColumn>{t("apiKeys.scopes")}</TableColumn>
                  <TableColumn>{t("common.active")}</TableColumn>
                  <TableColumn>{t("common.actions")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("common.empty")}>
                  {apiKeys.map((key) => (
                    <TableRow key={key.id}>
                      <TableCell>{key.name}</TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-1">
                          {key.scopes.slice(0, 3).map((scope) => (
                            <Chip key={scope} size="sm" variant="flat">
                              {scope}
                            </Chip>
                          ))}
                          {key.scopes.length > 3 && (
                            <Chip size="sm" variant="flat">+{key.scopes.length - 3}</Chip>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>{key.is_active ? t("common.enabled") : t("common.disabled")}</TableCell>
                      <TableCell>
                        <Button
                          size="sm"
                          color="danger"
                          variant="flat"
                          onPress={() => setDeleteId(key.id)}
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
        title={t("apiKeys.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("apiKeys.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("apiKeys.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={form.name}
              onChange={(e) => setForm({ ...form, name: e.target.value })}
              isRequired
            />
            <Textarea
              label={t("apiKeys.scopes")}
              value={form.scopes}
              onChange={(e) => setForm({ ...form, scopes: e.target.value })}
              className="font-mono"
            />
            <Textarea
              label={t("apiKeys.ipAllowlist")}
              value={form.ip_allowlist}
              onChange={(e) => setForm({ ...form, ip_allowlist: e.target.value })}
              className="font-mono"
            />
            <Input
              type="number"
              label={t("apiKeys.rateLimit")}
              value={form.rate_limit}
              onChange={(e) => setForm({ ...form, rate_limit: e.target.value })}
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
    </div>
  );
}
