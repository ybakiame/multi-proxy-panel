import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Button,
  Card,
  CardBody,
  Checkbox,
  Input,
  Modal,
  ModalBody,
  ModalContent,
  ModalFooter,
  ModalHeader,
  Select,
  SelectItem,
  Spinner,
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  Textarea,
} from "@heroui/react";
import { ConfirmDialog, CopyableSecret, Pagination } from "../components/ui";
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
  const { page, perPage, setPage, setPerPage, total, setTotal, totalPages } = usePagination();
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [templates, setTemplates] = useState<SubscriptionTemplate[]>([]);
  const [loading, setLoading] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [templateCreateOpen, setTemplateCreateOpen] = useState(false);
  const [editSubscription, setEditSubscription] = useState<Subscription | null>(null);
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
          <Button color="primary" onPress={() => { setNewToken(null); resetForm(); setCreateOpen(true); }}>
            {t("subscriptions.create")}
          </Button>
          <Button variant="flat" onPress={() => { resetTemplateForm(); setTemplateCreateOpen(true); }}>
            {t("subscriptions.createTemplate")}
          </Button>
        </div>
      </div>

      {newToken && (
        <CopyableSecret
          secret={newToken}
          label={t("nodes.tokenWarning")}
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
              <Table removeWrapper aria-label="subscriptions">
                <TableHeader>
                  <TableColumn>{t("subscriptions.client")}</TableColumn>
                  <TableColumn>{t("subscriptions.template")}</TableColumn>
                  <TableColumn>{t("subscriptions.token")}</TableColumn>
                  <TableColumn>{t("subscriptions.urlPath")}</TableColumn>
                  <TableColumn>{t("subscriptions.isActive")}</TableColumn>
                  <TableColumn>{t("subscriptions.expiresAt")}</TableColumn>
                  <TableColumn>{t("subscriptions.lastAccessed")}</TableColumn>
                  <TableColumn>{t("common.actions")}</TableColumn>
                </TableHeader>
                <TableBody emptyContent={t("common.empty")}>
                  {subscriptions.map((sub) => (
                    <TableRow key={sub.id}>
                      <TableCell>{clientName(sub.client_id)}</TableCell>
                      <TableCell>{templateName(sub.template_id)}</TableCell>
                      <TableCell>{maskToken(sub.token)}</TableCell>
                      <TableCell>{sub.url_path}</TableCell>
                      <TableCell>{sub.is_active ? t("common.enabled") : t("common.disabled")}</TableCell>
                      <TableCell>{formatDateTime(sub.expire_at)}</TableCell>
                      <TableCell>{formatDateTime(sub.last_accessed_at)}</TableCell>
                      <TableCell>
                        <div className="flex gap-2">
                          <Button size="sm" variant="flat" onPress={() => openEdit(sub)}>
                            {t("common.edit")}
                          </Button>
                          <Button
                            size="sm"
                            color="danger"
                            variant="flat"
                            onPress={() => setDeleteId(sub.id)}
                          >
                            {t("common.delete")}
                          </Button>
                        </div>
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
        title={t("subscriptions.deleteTitle")}
        isOpen={!!deleteId}
        onClose={() => setDeleteId(null)}
        onConfirm={handleDelete}
      >
        {t("subscriptions.deleteConfirm")}
      </ConfirmDialog>

      <Modal isOpen={createOpen} onClose={() => setCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("subscriptions.createTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Select
              label={t("subscriptions.client")}
              selectedKeys={form.client_id ? [form.client_id] : []}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, client_id: value || "" });
              }}
              isRequired
            >
              {clients.map((client) => (
                <SelectItem key={client.id}>{client.name}</SelectItem>
              ))}
            </Select>
            <Select
              label={t("subscriptions.template")}
              selectedKeys={form.template_id ? [form.template_id] : []}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setForm({ ...form, template_id: value || "" });
              }}
              isRequired
            >
              {templates.map((template) => (
                <SelectItem key={template.id}>{template.name}</SelectItem>
              ))}
            </Select>
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

      <Modal isOpen={!!editSubscription} onClose={() => setEditSubscription(null)}>
        <ModalContent>
          <ModalHeader>{t("subscriptions.editTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Checkbox
              isSelected={editForm.is_active}
              onValueChange={(selected) => setEditForm({ ...editForm, is_active: selected })}
            >
              {t("subscriptions.isActive")}
            </Checkbox>
            <Input
              type="datetime-local"
              label={t("subscriptions.expiresAt")}
              value={editForm.expire_at}
              onChange={(e) => setEditForm({ ...editForm, expire_at: e.target.value })}
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setEditSubscription(null)}>
              {t("common.cancel")}
            </Button>
            <Button color="primary" onPress={handleUpdate}>
              {t("common.update")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      <Modal isOpen={templateCreateOpen} onClose={() => setTemplateCreateOpen(false)}>
        <ModalContent>
          <ModalHeader>{t("subscriptions.templateCreateTitle")}</ModalHeader>
          <ModalBody className="space-y-4">
            <Input
              label={t("common.name")}
              value={templateForm.name}
              onChange={(e) => setTemplateForm({ ...templateForm, name: e.target.value })}
              isRequired
            />
            <Select
              label={t("subscriptions.format")}
              selectedKeys={[templateForm.format]}
              onSelectionChange={(keys) => {
                const value = Array.from(keys)[0] as string;
                setTemplateForm({ ...templateForm, format: value || "base64" });
              }}
            >
              {FORMAT_OPTIONS.map((format) => (
                <SelectItem key={format}>{format}</SelectItem>
              ))}
            </Select>
            <Textarea
              label={t("subscriptions.baseConfig")}
              value={templateForm.base_config}
              onChange={(e) => setTemplateForm({ ...templateForm, base_config: e.target.value })}
              className="font-mono"
            />
            <Textarea
              label={t("subscriptions.filterRules")}
              value={templateForm.filter_rules}
              onChange={(e) => setTemplateForm({ ...templateForm, filter_rules: e.target.value })}
              className="font-mono"
            />
            <Textarea
              label={t("subscriptions.customHeaders")}
              value={templateForm.custom_headers}
              onChange={(e) => setTemplateForm({ ...templateForm, custom_headers: e.target.value })}
              className="font-mono"
            />
          </ModalBody>
          <ModalFooter>
            <Button variant="flat" onPress={() => setTemplateCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button color="primary" onPress={handleCreateTemplate}>
              {t("common.create")}
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}
