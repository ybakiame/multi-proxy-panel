import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import * as yaml from "js-yaml";
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
import { Switch } from "../components/ui/switch";
import { TableCell, TableHead, TableRow } from "../components/ui/table";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "../components/ui/tabs";
import {
  ConfirmDialog,
  CopyableSecret,
  DataTable,
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
import { Subscription, SubscriptionTemplate } from "../api/types";
import { formatDateTime } from "../utils/format";
import { baseUrl } from "../api/config";

const FORMAT_OPTIONS = ["base64", "json", "clash", "sing-box", "v2rayng"];
const QR_FORMATS = [
  { id: "base64", label: "Base64" },
  { id: "clash", label: "Clash" },
  { id: "sing-box", label: "Sing-box" },
  { id: "v2rayng", label: "V2RayNG" },
];

const subscriptionsQueryKey = "subscriptions";
const templatesQueryKey = "templates";
const clientsQueryKey = "clients";

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
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState("subscriptions");

  // Subscriptions state
  const subsPagination = usePagination();
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
  const [templateFormOpen, setTemplateFormOpen] = useState(false);
  const [editTemplate, setEditTemplate] = useState<SubscriptionTemplate | null>(null);
  const [deleteTemplateId, setDeleteTemplateId] = useState<string | null>(null);
  const [templateForm, setTemplateForm] = useState({
    name: "",
    format: "base64",
    base_config: "",
    filter_rules: "{}",
    custom_headers: "{}",
    is_enabled: true,
  });

  // QR state
  const [qrSub, setQrSub] = useState<Subscription | null>(null);
  const [qrFormat, setQrFormat] = useState("base64");

  const { data: subsData, isLoading: loadingSubs } = useQuery({
    queryKey: [
      subscriptionsQueryKey,
      { page: subsPagination.page, perPage: subsPagination.perPage },
    ],
    queryFn: () => getSubscriptions(subsPagination.page, subsPagination.perPage),
    enabled: activeTab === "subscriptions",
  });

  const subscriptions = subsData?.data ?? [];
  const subsTotal = subsData?.pagination.total ?? 0;

  const { data: templates = [], isLoading: loadingTemplates } = useQuery({
    queryKey: [templatesQueryKey],
    queryFn: getTemplates,
    enabled: activeTab === "templates",
  });

  const { data: clientsData } = useQuery({
    queryKey: [clientsQueryKey, { page: 1, perPage: 1000 }],
    queryFn: () => getClients(1, 1000),
  });

  const clients = clientsData?.data ?? [];

  const createSubMutation = useMutation({
    mutationFn: createSubscription,
    onSuccess: (res) => {
      setNewToken(res.token || null);
      setCreateOpen(false);
      resetSubForm();
      queryClient.invalidateQueries({ queryKey: [subscriptionsQueryKey] });
    },
  });

  const updateSubMutation = useMutation({
    mutationFn: (payload: { id: string; data: Parameters<typeof updateSubscription>[1] }) =>
      updateSubscription(payload.id, payload.data),
    onSuccess: () => {
      setEditSubscription(null);
      queryClient.invalidateQueries({ queryKey: [subscriptionsQueryKey] });
    },
  });

  const deleteSubMutation = useMutation({
    mutationFn: deleteSubscription,
    onSuccess: () => {
      setDeleteSubId(null);
      queryClient.invalidateQueries({ queryKey: [subscriptionsQueryKey] });
    },
  });

  const saveTemplateMutation = useMutation({
    mutationFn: (payload: {
      id?: string;
      data: Parameters<typeof createTemplate>[0] | Parameters<typeof updateTemplate>[1];
    }) => {
      if (payload.id) {
        return updateTemplate(payload.id, payload.data as Parameters<typeof updateTemplate>[1]);
      }
      return createTemplate(payload.data as Parameters<typeof createTemplate>[0]);
    },
    onSuccess: () => {
      setTemplateFormOpen(false);
      resetTemplateFormState();
      setEditTemplate(null);
      queryClient.invalidateQueries({ queryKey: [templatesQueryKey] });
    },
  });

  const deleteTemplateMutation = useMutation({
    mutationFn: deleteTemplate,
    onSuccess: () => {
      setDeleteTemplateId(null);
      queryClient.invalidateQueries({ queryKey: [templatesQueryKey] });
    },
  });

  const toggleTemplateMutation = useMutation({
    mutationFn: (payload: { id: string; data: Parameters<typeof updateTemplate>[1] }) =>
      updateTemplate(payload.id, payload.data),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [templatesQueryKey] });
    },
  });

  // Subscription handlers
  const resetSubForm = () => setSubForm({ client_id: "" });

  const handleCreateSubscription = () => {
    createSubMutation.mutate({ client_id: subForm.client_id });
  };

  const openEditSubscription = (sub: Subscription) => {
    setEditSubscription(sub);
    setSubEditForm({
      is_active: sub.is_active,
      expire_at: toDatetimeLocalValue(sub.expire_at),
    });
  };

  const handleUpdateSubscription = () => {
    if (!editSubscription) return;
    updateSubMutation.mutate({
      id: editSubscription.id,
      data: {
        is_active: subEditForm.is_active,
        expire_at: subEditForm.expire_at || undefined,
      },
    });
  };

  const handleDeleteSubscription = () => {
    if (!deleteSubId) return;
    deleteSubMutation.mutate(deleteSubId);
  };

  // Template handlers
  const resetTemplateFormState = () => {
    setTemplateForm({
      name: "",
      format: "base64",
      base_config: "",
      filter_rules: "{}",
      custom_headers: "{}",
      is_enabled: true,
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
    if (tmpl.is_builtin) {
      // Builtin templates only allow enabling/disabling.
      setTemplateForm({
        name: tmpl.name,
        format: tmpl.format,
        base_config: baseConfig,
        filter_rules: "{}",
        custom_headers: "{}",
        is_enabled: tmpl.is_enabled,
      });
    } else {
      setTemplateForm({
        name: tmpl.name,
        format: tmpl.format,
        base_config: baseConfig,
        filter_rules: formatJson(tmpl.filter_rules),
        custom_headers: formatJson(tmpl.custom_headers),
        is_enabled: tmpl.is_enabled,
      });
    }
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

  const handleSaveTemplate = () => {
    const { filterRules, customHeaders } = parseTemplateFormJson();
    if (editTemplate) {
      const payload = editTemplate.is_builtin
        ? { is_enabled: templateForm.is_enabled }
        : {
            name: templateForm.name,
            format: templateForm.format,
            base_config: templateForm.base_config,
            filter_rules: filterRules,
            custom_headers: customHeaders,
            is_enabled: templateForm.is_enabled,
          };
      saveTemplateMutation.mutate({ id: editTemplate.id, data: payload });
    } else {
      saveTemplateMutation.mutate({
        data: {
          name: templateForm.name,
          format: templateForm.format,
          base_config: templateForm.base_config,
          filter_rules: filterRules,
          custom_headers: customHeaders,
          is_enabled: templateForm.is_enabled,
        },
      });
    }
  };

  const handleDeleteTemplate = () => {
    if (!deleteTemplateId) return;
    deleteTemplateMutation.mutate(deleteTemplateId);
  };

  const handleToggleTemplate = (tmpl: SubscriptionTemplate, selected: boolean) => {
    toggleTemplateMutation.mutate({ id: tmpl.id, data: { is_enabled: selected } });
  };

  // Helpers
  const clientName = (clientId: string) => {
    const client = clients.find((c) => c.id === clientId);
    return client?.name || clientId;
  };

  const templateFormatBadge = (format: string) => {
    const classNames: Record<string, string> = {
      "sing-box": "bg-success/10 text-success",
      v2rayng: "bg-warning/10 text-warning",
    };
    const variant = ["json", "sing-box", "v2rayng"].includes(format) ? "secondary" : "default";
    return (
      <Badge variant={variant} className={classNames[format]}>
        {format}
      </Badge>
    );
  };

  const templateDisplayName = (tmpl: SubscriptionTemplate) => {
    if (!tmpl.is_builtin) return tmpl.name;
    if (tmpl.format === "sing-box") return t("subscriptions.builtinSingBox");
    if (tmpl.format === "clash") return t("subscriptions.builtinClash");
    return tmpl.name;
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
      <Tabs value={activeTab} onValueChange={setActiveTab}>
        <TabsList aria-label="subscription tabs">
          <TabsTrigger value="subscriptions">{t("subscriptions.title")}</TabsTrigger>
          <TabsTrigger value="templates">{t("subscriptions.templates")}</TabsTrigger>
        </TabsList>
        <TabsContent value="subscriptions">
          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <h1 className="text-2xl font-bold">{t("subscriptions.title")}</h1>
              <Button
                onClick={() => {
                  setNewToken(null);
                  resetSubForm();
                  setCreateOpen(true);
                }}
              >
                {t("subscriptions.create")}
              </Button>
            </div>

            {newToken && (
              <CopyableSecret secret={newToken} label={t("subscriptions.tokenWarning")} />
            )}

            <Card>
              <CardContent>
                <DataTable
                  aria-label="subscriptions"
                  columns={7}
                  loading={loadingSubs}
                  empty={subscriptions.length === 0 ? t("common.empty") : undefined}
                  header={
                    <TableRow>
                      <TableHead>{t("subscriptions.client")}</TableHead>
                      <TableHead>{t("subscriptions.token")}</TableHead>
                      <TableHead>{t("subscriptions.urlPath")}</TableHead>
                      <TableHead>{t("subscriptions.isActive")}</TableHead>
                      <TableHead>{t("subscriptions.expiresAt")}</TableHead>
                      <TableHead>{t("subscriptions.lastAccessed")}</TableHead>
                      <TableHead>{t("common.actions")}</TableHead>
                    </TableRow>
                  }
                >
                  {subscriptions.map((sub) => (
                    <TableRow key={sub.id}>
                      <TableCell>{clientName(sub.client_id)}</TableCell>
                      <TableCell>{maskToken(sub.token)}</TableCell>
                      <TableCell className="max-w-xs truncate">{sub.url_path}</TableCell>
                      <TableCell>
                        {sub.is_active ? t("common.enabled") : t("common.disabled")}
                      </TableCell>
                      <TableCell>{formatDateTime(sub.expire_at)}</TableCell>
                      <TableCell>{formatDateTime(sub.last_accessed_at)}</TableCell>
                      <TableCell>
                        <div className="flex flex-wrap gap-2">
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => copyToClipboard(buildSubUrl(sub))}
                          >
                            {t("subscriptions.copyLink")}
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => {
                              setQrSub(sub);
                              setQrFormat("base64");
                            }}
                          >
                            {t("subscriptions.qrCode")}
                          </Button>
                          <Button
                            size="sm"
                            variant="ghost"
                            onClick={() => openEditSubscription(sub)}
                          >
                            {t("common.edit")}
                          </Button>
                          <Button
                            size="sm"
                            variant="destructive"
                            onClick={() => setDeleteSubId(sub.id)}
                          >
                            {t("common.delete")}
                          </Button>
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </DataTable>
                <Pagination
                  page={subsPagination.page}
                  totalPages={Math.max(1, Math.ceil(subsTotal / subsPagination.perPage))}
                  perPage={subsPagination.perPage}
                  total={subsTotal}
                  onPageChange={subsPagination.setPage}
                  onPerPageChange={subsPagination.setPerPage}
                />
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="templates">
          <div className="mt-4 space-y-4">
            <div className="flex items-center justify-between">
              <h1 className="text-2xl font-bold">{t("subscriptions.templates")}</h1>
              <Button onClick={openCreateTemplate}>{t("subscriptions.createTemplate")}</Button>
            </div>

            <Card>
              <CardContent>
                <DataTable
                  aria-label="subscription templates"
                  columns={5}
                  loading={loadingTemplates}
                  empty={templates.length === 0 ? t("common.empty") : undefined}
                  header={
                    <TableRow>
                      <TableHead>{t("common.name")}</TableHead>
                      <TableHead>{t("subscriptions.format")}</TableHead>
                      <TableHead>{t("subscriptions.builtin")}</TableHead>
                      <TableHead>{t("subscriptions.enabled")}</TableHead>
                      <TableHead>{t("common.actions")}</TableHead>
                    </TableRow>
                  }
                >
                  {templates.map((tmpl) => (
                    <TableRow key={tmpl.id}>
                      <TableCell>
                        <div className="flex items-center gap-2">
                          {templateDisplayName(tmpl)}
                          {tmpl.is_builtin && (
                            <Badge variant="secondary" className="bg-warning/10 text-warning">
                              {t("subscriptions.defaultTemplate")}
                            </Badge>
                          )}
                        </div>
                      </TableCell>
                      <TableCell>{templateFormatBadge(tmpl.format)}</TableCell>
                      <TableCell>{tmpl.is_builtin ? t("common.yes") : t("common.no")}</TableCell>
                      <TableCell>
                        <Switch
                          checked={tmpl.is_enabled}
                          onCheckedChange={(selected) => handleToggleTemplate(tmpl, selected)}
                          aria-label={t("subscriptions.enabled")}
                        />
                      </TableCell>
                      <TableCell>
                        <div className="flex gap-2">
                          <Button size="sm" variant="ghost" onClick={() => openEditTemplate(tmpl)}>
                            {t("common.edit")}
                          </Button>
                          {!tmpl.is_builtin && (
                            <Button
                              size="sm"
                              variant="destructive"
                              onClick={() => setDeleteTemplateId(tmpl.id)}
                            >
                              {t("common.delete")}
                            </Button>
                          )}
                        </div>
                      </TableCell>
                    </TableRow>
                  ))}
                </DataTable>
              </CardContent>
            </Card>
          </div>
        </TabsContent>
      </Tabs>

      {/* Create subscription modal */}
      <Dialog open={createOpen} onOpenChange={setCreateOpen}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.createTitle")}</DialogTitle>
          </DialogHeader>
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
          <DialogFooter>
            <Button variant="ghost" onClick={() => setCreateOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleCreateSubscription}>{t("common.create")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Edit subscription modal */}
      <Dialog
        open={!!editSubscription}
        onOpenChange={(open) => {
          if (!open) setEditSubscription(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.editTitle")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setEditSubscription(null)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleUpdateSubscription}>{t("common.update")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Template form modal */}
      <Dialog
        open={templateFormOpen}
        onOpenChange={(open) => {
          setTemplateFormOpen(open);
          if (!open) {
            setEditTemplate(null);
            resetTemplateFormState();
          }
        }}
      >
        <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>
              {editTemplate
                ? t("subscriptions.templateEditTitle")
                : t("subscriptions.templateCreateTitle")}
            </DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            {editTemplate?.is_builtin && (
              <p className="text-sm text-muted-foreground">{t("subscriptions.builtinReadonly")}</p>
            )}
            <FormInput
              label={t("common.name")}
              value={templateForm.name}
              onChange={(value) => setTemplateForm({ ...templateForm, name: value })}
              isRequired
              isReadOnly={editTemplate?.is_builtin}
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
              isDisabled={editTemplate?.is_builtin}
            />
            <div className="flex items-center gap-2">
              <Switch
                checked={templateForm.is_enabled}
                onCheckedChange={(selected) =>
                  setTemplateForm({ ...templateForm, is_enabled: selected })
                }
                aria-label={t("subscriptions.enabled")}
              />
              <span className="text-sm">
                {templateForm.is_enabled ? t("common.enabled") : t("common.disabled")}
              </span>
            </div>
            <CodeEditor
              label={t("subscriptions.baseConfig")}
              value={templateForm.base_config}
              onChange={(value) => setTemplateForm({ ...templateForm, base_config: value })}
              language={templateLanguage(templateForm.format)}
              readOnly={editTemplate?.is_builtin}
            />
            {!editTemplate?.is_builtin && (
              <>
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
              </>
            )}
          </div>
          <DialogFooter>
            <Button variant="ghost" onClick={() => setTemplateFormOpen(false)}>
              {t("common.cancel")}
            </Button>
            <Button onClick={handleSaveTemplate}>
              {editTemplate ? t("common.update") : t("common.create")}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* QR modal */}
      <Dialog
        open={!!qrSub}
        onOpenChange={(open) => {
          if (!open) setQrSub(null);
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{t("subscriptions.qrCode")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
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
          </div>
          <DialogFooter>
            <Button
              variant="ghost"
              onClick={() => qrSub && copyToClipboard(buildSubUrl(qrSub, qrFormat))}
            >
              {t("subscriptions.copyLink")}
            </Button>
            <Button onClick={() => setQrSub(null)}>{t("common.close")}</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

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
