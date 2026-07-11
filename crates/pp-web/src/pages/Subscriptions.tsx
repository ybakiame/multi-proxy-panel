import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, Card, Badge, Modal, Spinner, Table, Tabs } from "@heroui/react";
import * as yaml from "js-yaml";
import {
  ConfirmDialog,
  CopyableSecret,
  Pagination,
  FormInput,
  FormSelect,
  FormTextArea,
  FormCheckbox,
  CodeEditor,
} from "../components/ui";
import { usePagination } from "../hooks/useCommon";
import {
  getSubscriptions,
  createSubscription,
  updateSubscription,
  deleteSubscription,
  getTemplates,
  createTemplate,
  updateTemplate,
  deleteTemplate,
} from "../api/subscriptions";
import { getClients } from "../api/clients";
import { Client, Subscription, SubscriptionTemplate } from "../api/types";
import { formatDateTime } from "../utils/format";
import { baseUrl } from "../api/config";

const FORMAT_OPTIONS = ["base64", "json", "clash", "sing-box", "v2rayng"];
const QR_FORMATS = [
  { id: "base64", label: "Base64" },
  { id: "clash", label: "Clash" },
  { id: "sing-box", label: "Sing-box" },
  { id: "v2rayng", label: "V2RayNG" },
];

function maskToken(token: string) {
  if (!token) return "-";
  return `${token.slice(0, 8)}…`;
}

function toDatetimeLocalValue(iso: string | null) {
  if (!iso) return "";
  return iso.slice(0, 16);
}

function formatJson(obj: Record<string, unknown> | null) {
  return JSON.stringify(obj || {}, null, 2);
}

function isJsonLike(value: string): boolean {
  const trimmed = value.trim();
  return trimmed.startsWith("{") || trimmed.startsWith("[");
}

function defaultTemplate(format: string) {
  switch (format) {
    case "clash":
      return `port: 7890
socks-port: 7891
allow-lan: false
mode: rule
log-level: info
external-controller: 127.0.0.1:9090
# Auto-generated proxy list
proxies:
  <PROXY_REPLACE>
proxy-groups:
  - name: Proxy
    type: select
    proxies:
      <NODE_REPLACE>
rules:
  - MATCH,Proxy
`;
    case "sing-box":
      return JSON.stringify(
        {
          log: { level: "info" },
          dns: { servers: [{ tag: "local", address: "local" }] },
          inbounds: [{ type: "mixed", tag: "mixed-in", listen: "127.0.0.1", listen_port: 7890 }],
          outbounds: ["<OUTBOUND_REPLACE>", { type: "direct", tag: "direct" }],
          route: { final: "Proxy" },
        },
        null,
        2,
      );
    default:
      return "";
  }
}

function normalizeClashTemplate(value: string): string {
  if (!value) return value;
  if (!isJsonLike(value)) return value;
  try {
    const parsed = JSON.parse(value);
    const stringified = JSON.stringify(parsed);
    const hasProxyPlaceholder = stringified.includes("<PROXY_REPLACE>");
    const hasNodePlaceholder = stringified.includes("<NODE_REPLACE>");
    if (hasProxyPlaceholder || hasNodePlaceholder) {
      return yaml.dump(parsed, { indent: 2, lineWidth: -1 });
    }
    // Old JSON template without placeholders: merge into default YAML template
    let result = defaultTemplate("clash");
    if (Array.isArray(parsed.proxies) && parsed.proxies.length > 0) {
      const proxiesYaml = yaml.dump(parsed.proxies, { indent: 2, lineWidth: -1 });
      result = result.replace("  <PROXY_REPLACE>\n", proxiesYaml);
    }
    if (Array.isArray(parsed["proxy-groups"]) && parsed["proxy-groups"].length > 0) {
      const groupNames = parsed["proxy-groups"]
        .map((g: { name?: string }) => g.name)
        .filter(Boolean);
      const namesYaml = yaml.dump(groupNames, { indent: 2, lineWidth: -1 });
      result = result.replace("      <NODE_REPLACE>\n", namesYaml);
    }
    return result;
  } catch {
    return value;
  }
}

function normalizeTemplateForFormat(value: string, format: string, isCreate: boolean): string {
  if (!value) {
    return isCreate ? defaultTemplate(format) : "";
  }
  if (format === "clash") {
    return normalizeClashTemplate(value);
  }
  if (["sing-box", "json", "v2rayng"].includes(format)) {
    // If the current content is YAML, try to convert to JSON
    if (!isJsonLike(value)) {
      try {
        const parsed = yaml.load(value);
        return JSON.stringify(parsed, null, 2);
      } catch {
        return value;
      }
    }
  }
  return value;
}

export function Subscriptions() {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState("subscriptions");

  // Subscriptions state
  const subsPagination = usePagination();
  const [subscriptions, setSubscriptions] = useState<Subscription[]>([]);
  const [clients, setClients] = useState<Client[]>([]);
  const [templates, setTemplates] = useState<SubscriptionTemplate[]>([]);
  const [loadingSubs, setLoadingSubs] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [editSubscription, setEditSubscription] = useState<Subscription | null>(null);
  const [deleteSubId, setDeleteSubId] = useState<string | null>(null);
  const [newToken, setNewToken] = useState<string | null>(null);
  const [subForm, setSubForm] = useState({ client_id: "" });
  const [subEditForm, setSubEditForm] = useState({
    is_active: true,
    expire_at: "",
  });

  // Templates state
  const [loadingTemplates, setLoadingTemplates] = useState(false);
  const [templateFormOpen, setTemplateFormOpen] = useState(false);
  const [editTemplate, setEditTemplate] = useState<SubscriptionTemplate | null>(null);
  const [deleteTemplateId, setDeleteTemplateId] = useState<string | null>(null);
  const [templateForm, setTemplateForm] = useState({
    name: "",
    format: "base64",
    base_config: "",
    filter_rules: "{}",
    custom_headers: "{}",
  });

  // QR state
  const [qrSub, setQrSub] = useState<Subscription | null>(null);
  const [qrFormat, setQrFormat] = useState("base64");

  const fetchSubscriptions = async () => {
    setLoadingSubs(true);
    try {
      const res = await getSubscriptions(subsPagination.page, subsPagination.perPage);
      setSubscriptions(res.data);
      subsPagination.setTotal(res.pagination.total);
    } finally {
      setLoadingSubs(false);
    }
  };

  const fetchTemplates = async () => {
    setLoadingTemplates(true);
    try {
      const res = await getTemplates();
      setTemplates(res);
    } finally {
      setLoadingTemplates(false);
    }
  };

  const fetchClients = async () => {
    try {
      const res = await getClients(1, 1000);
      setClients(res.data);
    } catch {
      // handled by interceptor
    }
  };

  useEffect(() => {
    fetchClients();
    fetchTemplates();
  }, []);

  useEffect(() => {
    if (activeTab === "subscriptions") {
      fetchSubscriptions();
    }
  }, [activeTab, subsPagination.page, subsPagination.perPage]);

  useEffect(() => {
    if (activeTab === "templates") {
      fetchTemplates();
    }
  }, [activeTab]);

  // Subscription handlers
  const resetSubForm = () => setSubForm({ client_id: "" });

  const handleCreateSubscription = async () => {
    try {
      const res = await createSubscription({ client_id: subForm.client_id });
      setNewToken(res.token || null);
      setCreateOpen(false);
      resetSubForm();
      fetchSubscriptions();
    } catch {
      // handled by interceptor
    }
  };

  const openEditSubscription = (sub: Subscription) => {
    setEditSubscription(sub);
    setSubEditForm({
      is_active: sub.is_active,
      expire_at: toDatetimeLocalValue(sub.expire_at),
    });
  };

  const handleUpdateSubscription = async () => {
    if (!editSubscription) return;
    try {
      await updateSubscription(editSubscription.id, {
        is_active: subEditForm.is_active,
        expire_at: subEditForm.expire_at || undefined,
      });
      setEditSubscription(null);
      fetchSubscriptions();
    } catch {
      // handled by interceptor
    }
  };

  const handleDeleteSubscription = async () => {
    if (!deleteSubId) return;
    try {
      await deleteSubscription(deleteSubId);
      setDeleteSubId(null);
      fetchSubscriptions();
    } catch {
      // handled by interceptor
    }
  };

  // Template handlers
  const resetTemplateFormState = () => {
    setTemplateForm({
      name: "",
      format: "base64",
      base_config: "",
      filter_rules: "{}",
      custom_headers: "{}",
    });
  };

  const openCreateTemplate = () => {
    setEditTemplate(null);
    resetTemplateFormState();
    setTemplateFormOpen(true);
  };

  const openEditTemplate = (tmpl: SubscriptionTemplate) => {
    setEditTemplate(tmpl);
    let baseConfig = tmpl.base_config || "";
    if (tmpl.format === "clash") {
      baseConfig = normalizeClashTemplate(baseConfig);
    }
    setTemplateForm({
      name: tmpl.name,
      format: tmpl.format,
      base_config: baseConfig,
      filter_rules: formatJson(tmpl.filter_rules),
      custom_headers: formatJson(tmpl.custom_headers),
    });
    setTemplateFormOpen(true);
  };

  const parseTemplateFormJson = () => {
    let filterRules: Record<string, unknown> = {};
    let customHeaders: Record<string, string> = {};
    try {
      filterRules = JSON.parse(templateForm.filter_rules);
    } catch {}
    try {
      customHeaders = JSON.parse(templateForm.custom_headers);
    } catch {}
    return { filterRules, customHeaders };
  };

  const templateLanguage = (format: string): "yaml" | "json" | "text" => {
    if (format === "clash") return "yaml";
    if (format === "sing-box" || format === "json" || format === "v2rayng") return "json";
    return "text";
  };

  const handleSaveTemplate = async () => {
    const { filterRules, customHeaders } = parseTemplateFormJson();
    const payload = {
      name: templateForm.name,
      format: templateForm.format,
      base_config: templateForm.base_config,
      filter_rules: filterRules,
      custom_headers: customHeaders,
    };
    try {
      if (editTemplate) {
        await updateTemplate(editTemplate.id, payload);
      } else {
        await createTemplate(payload);
      }
      setTemplateFormOpen(false);
      resetTemplateFormState();
      setEditTemplate(null);
      fetchTemplates();
    } catch {
      // handled by interceptor
    }
  };

  const handleDeleteTemplate = async () => {
    if (!deleteTemplateId) return;
    try {
      await deleteTemplate(deleteTemplateId);
      setDeleteTemplateId(null);
      fetchTemplates();
    } catch {
      // handled by interceptor
    }
  };

  // Helpers
  const clientName = (clientId: string) => {
    const client = clients.find((c) => c.id === clientId);
    return client?.name || clientId;
  };

  const templateFormatBadge = (format: string) => {
    const colors: Record<string, string> = {
      base64: "default",
      json: "secondary",
      clash: "primary",
      "sing-box": "success",
      v2rayng: "warning",
    };
    return (
      <Badge color={(colors[format] as never) || "default"} size="sm">
        {format}
      </Badge>
    );
  };

  const buildSubUrl = (sub: Subscription, format?: string) => {
    const origin = baseUrl() || window.location.origin;
    let url = `${origin}${sub.url_path}`;
    if (format) {
      url += `?format=${encodeURIComponent(format)}`;
    }
    return url;
  };

  const copyToClipboard = (text: string) => {
    navigator.clipboard.writeText(text).catch(() => {});
  };

  return (
    <div className="space-y-4">
      <Tabs
        aria-label="subscription tabs"
        selectedKey={activeTab}
        onSelectionChange={(key) => setActiveTab(key as string)}
      >
        <Tabs.ListContainer>
          <Tabs.List aria-label="subscription tabs">
            <Tabs.Tab id="subscriptions">{t("subscriptions.title")}</Tabs.Tab>
            <Tabs.Tab id="templates">{t("subscriptions.templates")}</Tabs.Tab>
          </Tabs.List>
        </Tabs.ListContainer>
        <Tabs.Panel id="subscriptions">
          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <h1 className="text-2xl font-bold">{t("subscriptions.title")}</h1>
              <Button
                onPress={() => {
                  setNewToken(null);
                  resetSubForm();
                  setCreateOpen(true);
                }}
              >
                {t("subscriptions.create")}
              </Button>
            </div>

            {newToken && <CopyableSecret secret={newToken} label={t("nodes.tokenWarning")} />}

            <Card>
              <Card.Content>
                {loadingSubs ? (
                  <div className="flex h-32 items-center justify-center">
                    <Spinner />
                  </div>
                ) : (
                  <>
                    <Table aria-label="subscriptions">
                      <Table.ScrollContainer>
                        <Table.Content>
                          <Table.Header>
                            <Table.Column isRowHeader>{t("subscriptions.client")}</Table.Column>
                            <Table.Column>{t("subscriptions.token")}</Table.Column>
                            <Table.Column>{t("subscriptions.urlPath")}</Table.Column>
                            <Table.Column>{t("subscriptions.isActive")}</Table.Column>
                            <Table.Column>{t("subscriptions.expiresAt")}</Table.Column>
                            <Table.Column>{t("subscriptions.lastAccessed")}</Table.Column>
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
                                <Table.Cell>{maskToken(sub.token)}</Table.Cell>
                                <Table.Cell className="max-w-xs truncate">
                                  {sub.url_path}
                                </Table.Cell>
                                <Table.Cell>
                                  {sub.is_active ? t("common.enabled") : t("common.disabled")}
                                </Table.Cell>
                                <Table.Cell>{formatDateTime(sub.expire_at)}</Table.Cell>
                                <Table.Cell>{formatDateTime(sub.last_accessed_at)}</Table.Cell>
                                <Table.Cell>
                                  <div className="flex flex-wrap gap-2">
                                    <Button
                                      size="sm"
                                      variant="ghost"
                                      onPress={() => copyToClipboard(buildSubUrl(sub))}
                                    >
                                      {t("subscriptions.copyLink")}
                                    </Button>
                                    <Button
                                      size="sm"
                                      variant="ghost"
                                      onPress={() => {
                                        setQrSub(sub);
                                        setQrFormat("base64");
                                      }}
                                    >
                                      {t("subscriptions.qrCode")}
                                    </Button>
                                    <Button
                                      size="sm"
                                      variant="ghost"
                                      onPress={() => openEditSubscription(sub)}
                                    >
                                      {t("common.edit")}
                                    </Button>
                                    <Button
                                      size="sm"
                                      variant="danger"
                                      onPress={() => setDeleteSubId(sub.id)}
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
                      page={subsPagination.page}
                      totalPages={subsPagination.totalPages}
                      perPage={subsPagination.perPage}
                      total={subsPagination.total}
                      onPageChange={subsPagination.setPage}
                      onPerPageChange={subsPagination.setPerPage}
                    />
                  </>
                )}
              </Card.Content>
            </Card>
          </div>
        </Tabs.Panel>

        <Tabs.Panel id="templates">
          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <h1 className="text-2xl font-bold">{t("subscriptions.templates")}</h1>
              <Button onPress={openCreateTemplate}>{t("subscriptions.createTemplate")}</Button>
            </div>

            <Card>
              <Card.Content>
                {loadingTemplates ? (
                  <div className="flex h-32 items-center justify-center">
                    <Spinner />
                  </div>
                ) : (
                  <Table aria-label="subscription templates">
                    <Table.ScrollContainer>
                      <Table.Content>
                        <Table.Header>
                          <Table.Column isRowHeader>{t("common.name")}</Table.Column>
                          <Table.Column>{t("subscriptions.format")}</Table.Column>
                          <Table.Column>{t("subscriptions.baseConfig")}</Table.Column>
                          <Table.Column>{t("subscriptions.filterRules")}</Table.Column>
                          <Table.Column>{t("subscriptions.customHeaders")}</Table.Column>
                          <Table.Column>{t("common.actions")}</Table.Column>
                        </Table.Header>
                        <Table.Body
                          renderEmptyState={() => (
                            <div className="p-4 text-center text-muted-foreground">
                              {t("common.empty")}
                            </div>
                          )}
                        >
                          {templates.map((tmpl) => (
                            <Table.Row key={tmpl.id}>
                              <Table.Cell>{tmpl.name}</Table.Cell>
                              <Table.Cell>{templateFormatBadge(tmpl.format)}</Table.Cell>
                              <Table.Cell>
                                {tmpl.base_config ? t("common.yes") : t("common.no")}
                              </Table.Cell>
                              <Table.Cell>
                                {tmpl.filter_rules ? t("common.yes") : t("common.no")}
                              </Table.Cell>
                              <Table.Cell>
                                {tmpl.custom_headers ? t("common.yes") : t("common.no")}
                              </Table.Cell>
                              <Table.Cell>
                                <div className="flex gap-2">
                                  <Button
                                    size="sm"
                                    variant="ghost"
                                    onPress={() => openEditTemplate(tmpl)}
                                  >
                                    {t("common.edit")}
                                  </Button>
                                  <Button
                                    size="sm"
                                    variant="danger"
                                    onPress={() => setDeleteTemplateId(tmpl.id)}
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
          </div>
        </Tabs.Panel>
      </Tabs>

      {/* Create subscription modal */}
      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("subscriptions.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("subscriptions.client")}
                value={subForm.client_id}
                onChange={(value) => setSubForm({ ...subForm, client_id: value })}
                options={clients.map((client) => ({
                  id: client.id,
                  label: client.name,
                }))}
                isRequired
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setCreateOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleCreateSubscription}>{t("common.create")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* Edit subscription modal */}
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
                isSelected={subEditForm.is_active}
                onChange={(selected) => setSubEditForm({ ...subEditForm, is_active: selected })}
              >
                {t("subscriptions.isActive")}
              </FormCheckbox>
              <FormInput
                type="datetime-local"
                label={t("subscriptions.expiresAt")}
                value={subEditForm.expire_at}
                onChange={(value) => setSubEditForm({ ...subEditForm, expire_at: value })}
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setEditSubscription(null)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleUpdateSubscription}>{t("common.update")}</Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* Template form modal */}
      <Modal.Backdrop
        isOpen={templateFormOpen}
        onOpenChange={(open) => {
          setTemplateFormOpen(open);
          if (!open) {
            setEditTemplate(null);
            resetTemplateFormState();
          }
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>
                {editTemplate
                  ? t("subscriptions.templateEditTitle")
                  : t("subscriptions.templateCreateTitle")}
              </Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={templateForm.name}
                onChange={(value) => setTemplateForm({ ...templateForm, name: value })}
                isRequired
              />
              <FormSelect
                label={t("subscriptions.format")}
                value={templateForm.format}
                onChange={(value) => {
                  const format = value || "base64";
                  setTemplateForm({
                    ...templateForm,
                    format,
                    base_config: normalizeTemplateForFormat(
                      templateForm.base_config,
                      format,
                      !editTemplate,
                    ),
                  });
                }}
                options={FORMAT_OPTIONS.map((format) => ({
                  id: format,
                  label: format,
                }))}
              />
              <CodeEditor
                label={t("subscriptions.baseConfig")}
                value={templateForm.base_config}
                onChange={(value) => setTemplateForm({ ...templateForm, base_config: value })}
                language={templateLanguage(templateForm.format)}
              />
              <FormTextArea
                label={t("subscriptions.filterRules")}
                value={templateForm.filter_rules}
                onChange={(value) => setTemplateForm({ ...templateForm, filter_rules: value })}
                className="font-mono"
              />
              <FormTextArea
                label={t("subscriptions.customHeaders")}
                value={templateForm.custom_headers}
                onChange={(value) => setTemplateForm({ ...templateForm, custom_headers: value })}
                className="font-mono"
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setTemplateFormOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button onPress={handleSaveTemplate}>
                {editTemplate ? t("common.update") : t("common.create")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* QR modal */}
      <Modal.Backdrop
        isOpen={!!qrSub}
        onOpenChange={(open) => {
          if (!open) setQrSub(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("subscriptions.qrCode")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormSelect
                label={t("subscriptions.format")}
                value={qrFormat}
                onChange={(value) => setQrFormat(value || "base64")}
                options={QR_FORMATS}
              />
              {qrSub && (
                <div className="flex flex-col items-center gap-3">
                  <img
                    src={`${baseUrl()}${qrSub.url_path}/qr?format=${encodeURIComponent(qrFormat)}`}
                    alt="subscription qr"
                    className="rounded border"
                  />
                  <p className="break-all text-center text-sm text-muted-foreground">
                    {buildSubUrl(qrSub, qrFormat)}
                  </p>
                </div>
              )}
            </Modal.Body>
            <Modal.Footer>
              <Button
                variant="ghost"
                onPress={() => qrSub && copyToClipboard(buildSubUrl(qrSub, qrFormat))}
              >
                {t("subscriptions.copyLink")}
              </Button>
              <Button slot="close" onPress={() => setQrSub(null)}>
                {t("common.close")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      <ConfirmDialog
        title={t("subscriptions.deleteTitle")}
        isOpen={!!deleteSubId}
        onClose={() => setDeleteSubId(null)}
        onConfirm={handleDeleteSubscription}
      >
        {t("subscriptions.deleteConfirm")}
      </ConfirmDialog>

      <ConfirmDialog
        title={t("subscriptions.templateDeleteTitle")}
        isOpen={!!deleteTemplateId}
        onClose={() => setDeleteTemplateId(null)}
        onConfirm={handleDeleteTemplate}
      >
        {t("subscriptions.templateDeleteConfirm")}
      </ConfirmDialog>
    </div>
  );
}
