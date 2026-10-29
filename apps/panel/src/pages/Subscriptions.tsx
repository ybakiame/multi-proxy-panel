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
import {
  ConfirmDialog,
  DataTable,
  FormInput,
  FormSelect,
  FormTextArea,
  CodeEditor,
} from "../components/ui";
import { getTemplates, createTemplate, updateTemplate, deleteTemplate } from "../api/subscriptions";
import { SubscriptionTemplate } from "../api/types";

const FORMAT_OPTIONS = ["base64", "json", "clash", "sing-box", "v2rayng"];
const templatesQueryKey = "templates";

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
  if (!value || !isJsonLike(value)) return value;
  try {
    const parsed = JSON.parse(value);
    const stringified = JSON.stringify(parsed);
    const hasProxyPlaceholder = stringified.includes("<PROXY_REPLACE>");
    const hasNodePlaceholder = stringified.includes("<NODE_REPLACE>");
    if (hasProxyPlaceholder || hasNodePlaceholder) {
      return yaml.dump(parsed, { indent: 2, lineWidth: -1 });
    }
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
  if (["sing-box", "json", "v2rayng"].includes(format) && !isJsonLike(value)) {
    try {
      const parsed = yaml.load(value);
      return JSON.stringify(parsed, null, 2);
    } catch {
      return value;
    }
  }
  return value;
}

export function Subscriptions() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
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

  const { data: templates = [], isLoading: loadingTemplates } = useQuery({
    queryKey: [templatesQueryKey],
    queryFn: getTemplates,
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

  return (
    <div className="space-y-4">
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
