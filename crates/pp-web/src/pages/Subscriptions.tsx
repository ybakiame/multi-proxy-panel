import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Badge, Modal, Spinner, Table } from "@heroui/react";
import {
  ConfirmDialog,
  CopyableSecret,
  Pagination,
  FormInput,
  FormSelect,
  FormTextArea,
  FormCheckbox,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getSubscriptions,
  createSubscription,
  updateSubscription,
  deleteSubscription,
  getTemplates,
  createTemplate,
} from "../api/subscriptions";
import { getClients } from "../api/clients";
import { Client, Subscription, SubscriptionTemplate } from "../api/types";
import { formatDateTime } from "../utils/format";

const FORMAT_OPTIONS = ["base64", "json", "clash", "sing-box", "v2rayng"];

function maskToken(token: string) {
  if (!token) return "-";
  return `${token.slice(0, 8)}…`;
}

function toDatetimeLocalValue(iso: string | null) {
  if (!iso) return "";
  return iso.slice(0, 16);
}

export function Subscriptions() {
  const { t } = useTranslation();
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } =
    usePagination();
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [templates, setTemplates] = useState<SubscriptionTemplate[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [templateCreateOpen, setTemplateCreateOpen] = useState(false);
  const [editSubscription, setEditSubscription] = useState<Subscription | null>(
    null,
  );
  const [deleteId, setDeleteId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [form, setForm] = useState({
    client_id: "",
    template_id: "",
  });
  const [editForm, setEditForm] = useState({
    is_active: true,
    expire_at: "",
  });
  const [templateForm, setTemplateForm] = useState({
    name: "",
    format: "base64",
    base_config: "{}",
    filter_rules: "{}",
    custom_headers: "{}",
  });

  const fetchData = async () => {
    setLoading(true);
    try {
      const [subsRes, clientsRes, templatesRes] = await Promise.all([
        getSubscriptions(page, perPage),
        getClients(1, 1000),
        getTemplates(),
      ]);
      setSubscriptions(subsRes.data);
      setTotal(subsRes.pagination.total);
      setClients(clientsRes.data);
      setTemplates(templatesRes);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    fetchData();
  }, [page, perPage]);

  const resetForm = () => {
    setForm({ client_id: "", template_id: "" });
  };

  const resetTemplateForm = () => {
    setTemplateForm({
      name: "",
      format: "base64",
      base_config: "{}",
      filter_rules: "{}",
      custom_headers: "{}",
    });
  };

  const handleCreate = async () => {
    try {
      const res = await createSubscription({
        client_id: form.client_id,
        template_id: form.template_id,
      });
      setNewToken(res.token || null);
      setCreateOpen(false);
      resetForm();
      fetchData();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleUpdate = async () => {
    if (!editSubscription) return;
    try {
      await updateSubscription(editSubscription.id, {
        is_active: editForm.is_active,
        expire_at: editForm.expire_at || undefined,
      });
      setEditSubscription(null);
      fetchData();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleDelete = async () => {
    if (!deleteId) return;
    try {
      await deleteSubscription(deleteId);
      setDeleteId(null);
      fetchData();
    } catch {
      // error handled by axios interceptor
    }
  };

  const handleCreateTemplate = async () => {
    try {
      let baseConfig: Record<string, unknown> = {};
      let filterRules: Record<string, unknown> = {};
      let customHeaders: Record<string, string> = {};
      try {
        baseConfig = JSON.parse(templateForm.base_config);
      } catch {}
      try {
        filterRules = JSON.parse(templateForm.filter_rules);
      } catch {}
      try {
        customHeaders = JSON.parse(templateForm.custom_headers);
      } catch {}
      await createTemplate({
        name: templateForm.name,
        format: templateForm.format,
        base_config: baseConfig,
        filter_rules: filterRules,
        custom_headers: customHeaders,
      });
      setTemplateCreateOpen(false);
      resetTemplateForm();
      const templatesRes = await getTemplates();
      setTemplates(templatesRes);
    } catch {
      // error handled by axios interceptor
    }
  };

  const openEdit = (sub: Subscription) => {
    setEditSubscription(sub);
    setEditForm({
      is_active: sub.is_active,
      expire_at: toDatetimeLocalValue(sub.expire_at),
    });
  };

  const clientName = (clientId: string) => {
    const client = clients.find((c) => c.id === clientId);
    return client?.name || clientId;
  };

  const templateName = (templateId: string) => {
    const template = templates.find((t) => t.id === templateId);
    return template?.name || templateId;
  };

  return (
    <div className="space-y-4">
      <div className="mb-6 flex items-center justify-between">
        <h1 className="text-2xl font-bold">{t("subscriptions.title")}</h1>
        <div className="flex gap-2">
          <Button
            onPress={() => {
              setNewToken(null);
              resetForm();
              setCreateOpen(true);
            }}
          >
            {t("subscriptions.create")}
          </Button>
          <Button
            variant="ghost"
            onPress={() => {
              resetTemplateForm();
              setTemplateCreateOpen(true);
            }}
          >
            {t("subscriptions.createTemplate")}
          </Button>
        </div>
      </div>

      {newToken && (
        <CopyableSecret secret={newToken} label={t("nodes.tokenWarning")} />
      )}

      <Card>
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <>
              <Table aria-label="subscriptions">
                <Table.ScrollContainer>
                  <Table.Content>
                    <Table.Header>
                      <Table.Column isRowHeader>
                        {t("subscriptions.client")}
                      </Table.Column>
                      <Table.Column>{t("subscriptions.template")}</Table.Column>
                      <Table.Column>{t("subscriptions.token")}</Table.Column>
                      <Table.Column>{t("subscriptions.urlPath")}</Table.Column>
                      <Table.Column>{t("subscriptions.isActive")}</Table.Column>
                      <Table.Column>
                        {t("subscriptions.expiresAt")}
                      </Table.Column>
                      <Table.Column>
                        {t("subscriptions.lastAccessed")}
                      </Table.Column>
                      <Table.Column>{t("common.actions")}</Table.Column>
                    </Table.Header>
                    <Table.Body
                      renderEmptyState={() => (
                        <div className="p-4 text-center text-muted-foreground">
                          {t("common.empty")}
                        </div>
                      )}
                    >
                      {subscriptions.map((sub) => (
                        <Table.Row key={sub.id}>
                          <Table.Cell>{clientName(sub.client_id)}</Table.Cell>
                          <Table.Cell>
                            {templateName(sub.template_id)}
                          </Table.Cell>
                          <Table.Cell>{maskToken(sub.token)}</Table.Cell>
                          <Table.Cell>{sub.url_path}</Table.Cell>
                          <Table.Cell>
                            {sub.is_active
                              ? t("common.enabled")
                              : t("common.disabled")}
                          </Table.Cell>
                          <Table.Cell>
                            {formatDateTime(sub.expire_at)}
                          </Table.Cell>
                          <Table.Cell>
                            {formatDateTime(sub.last_accessed_at)}
                          </Table.Cell>
                          <Table.Cell>
                            <div className="flex gap-2">
                              <Button
                                size="sm"
                                variant="ghost"
                                onPress={() => openEdit(sub)}
                              >
                                {t("common.edit")}
                              </Button>
                              <Button
                                size="sm"
                                variant="danger"
                                onPress={() => setDeleteId(sub.id)}
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
        title={t("subscriptions.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("subscriptions.deleteConfirm")}
      </ConfirmDialog>

      <Modal.Backdrop
        isOpen={createOpen}
        onOpenChange={(open) => setCreateOpen(open)}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("subscriptions.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("subscriptions.client")}
                value={form.client_id}
                onChange={(value) => setForm({ ...form, client_id: value })}
                options={clients.map((client) => ({
                  id: client.id,
                  label: client.name,
                }))}
                isRequired
              />
              <FormSelect
                label={t("subscriptions.template")}
                value={form.template_id}
                onChange={(value) => setForm({ ...form, template_id: value })}
                options={templates.map((template) => ({
                  id: template.id,
                  label: template.name,
                }))}
                isRequired
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

      <Modal.Backdrop
        isOpen={!!editSubscription}
        onOpenChange={(open) => {
          if (!open) setEditSubscription(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("subscriptions.editTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormCheckbox
                isSelected={editForm.is_active}
                onChange={(selected) =>
                  setEditForm({ ...editForm, is_active: selected })
                }
              >
                {t("subscriptions.isActive")}
              </FormCheckbox>
              <FormInput
                type="datetime-local"
                label={t("subscriptions.expiresAt")}
                value={editForm.expire_at}
                onChange={(value) =>
                  setEditForm({ ...editForm, expire_at: value })
                }
              />
            </Modal.Body>
            <Modal.Footer>
              <Button
                slot="close"
                variant="ghost"
                onPress={() => setEditSubscription(null)}
              >
                {t("common.cancel")}
              </Button>
              <Button onPress={handleUpdate}>{t("common.update")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <Modal.Backdrop
        isOpen={templateCreateOpen}
        onOpenChange={(open) => setTemplateCreateOpen(open)}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {t("subscriptions.templateCreateTitle")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={templateForm.name}
                onChange={(value) =>
                  setTemplateForm({ ...templateForm, name: value })
                }
                isRequired
              />
              <FormSelect
                label={t("subscriptions.format")}
                value={templateForm.format}
                onChange={(value) =>
                  setTemplateForm({
                    ...templateForm,
                    format: value || "base64",
                  })
                }
                options={FORMAT_OPTIONS.map((format) => ({
                  id: format,
                  label: format,
                }))}
              />
              <FormTextArea
                label={t("subscriptions.baseConfig")}
                value={templateForm.base_config}
                onChange={(value) =>
                  setTemplateForm({ ...templateForm, base_config: value })
                }
                className="font-mono"
              />
              <FormTextArea
                label={t("subscriptions.filterRules")}
                value={templateForm.filter_rules}
                onChange={(value) =>
                  setTemplateForm({ ...templateForm, filter_rules: value })
                }
                className="font-mono"
              />
              <FormTextArea
                label={t("subscriptions.customHeaders")}
                value={templateForm.custom_headers}
                onChange={(value) =>
                  setTemplateForm({ ...templateForm, custom_headers: value })
                }
                className="font-mono"
              />
            </Modal.Body>
            <Modal.Footer>
              <Button
                slot="close"
                variant="ghost"
                onPress={() => setTemplateCreateOpen(false)}
              >
                {t("common.cancel")}
              </Button>
              <Button onPress={handleCreateTemplate}>
                {t("common.create")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
