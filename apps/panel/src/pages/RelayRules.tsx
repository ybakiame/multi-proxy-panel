import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button } from "../components/ui/button";
import { Card, CardContent } from "../components/ui/card";
import { Spinner } from "../components/ui/spinner";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "../components/ui/table";
import { PageHeader, ConfirmDialog } from "../components/ui";
import {
  getRelayRules,
  getRuleSetLibrary,
  createRelayRule,
  updateRelayRule,
  deleteRelayRule,
  RelayRule,
} from "../api/relayRules";
import { getNodes } from "../api/nodes";
import { getBindings } from "../api/bindings";
import { getAllProtocols } from "../api/protocols";
import { RelayRuleDialog, type RelayRuleForm } from "./RelayRuleDialog";

const defaultForm: RelayRuleForm = {
  name: "",
  node_id: "",
  exit_binding_id: "",
  match_type: "inline",
  domains: "",
  domain_suffixes: "",
  library_name: "",
  singbox_url: "",
  mihomo_url: "",
  enabled: true,
  sort_order: "0",
};

const relayRulesQueryKey = "relay-rules";
const nodesQueryKey = "nodes";
const bindingsQueryKey = "bindings";
const protocolsQueryKey = "protocols";
const libraryQueryKey = "relay-rules-library";

function parseLines(text: string): string[] {
  return text
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);
}

function buildMatchConfig(form: RelayRuleForm): RelayRule["match_config"] {
  if (form.match_type === "inline") {
    return {
      domains: parseLines(form.domains),
      domain_suffixes: parseLines(form.domain_suffixes),
    };
  }
  if (form.library_name) {
    return { library: form.library_name };
  }
  return {
    custom: {
      singbox: form.singbox_url ? { url: form.singbox_url } : undefined,
      mihomo: form.mihomo_url ? { url: form.mihomo_url } : undefined,
    },
  };
}

function prefillForm(rule: RelayRule): RelayRuleForm {
  const base = {
    name: rule.name,
    node_id: rule.node_id,
    exit_binding_id: rule.exit_binding_id,
    match_type: rule.match_type,
    domains: "",
    domain_suffixes: "",
    library_name: "",
    singbox_url: "",
    mihomo_url: "",
    enabled: rule.enabled,
    sort_order: String(rule.sort_order),
  };
  if (rule.match_type === "inline") {
    return {
      ...base,
      domains: (rule.match_config.domains || []).join("\n"),
      domain_suffixes: (rule.match_config.domain_suffixes || []).join("\n"),
    };
  }
  if (rule.match_type === "rule_set") {
    if (rule.match_config.library) {
      return { ...base, library_name: rule.match_config.library };
    }
    return {
      ...base,
      library_name: "",
      singbox_url: rule.match_config.custom?.singbox?.url || "",
      mihomo_url: rule.match_config.custom?.mihomo?.url || "",
    };
  }
  return base;
}

export function RelayRules() {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [createOpen, setCreateOpen] = useState(false);
  const [editRule, setEditRule] = useState<RelayRule | null>(null);
  const [deleteRuleId, setDeleteRuleId] = useState<string | null>(null);
  const [form, setForm] = useState<RelayRuleForm>(defaultForm);

  const { data: rules = [], isLoading: rulesLoading } = useQuery({
    queryKey: [relayRulesQueryKey],
    queryFn: getRelayRules,
  });

  const { data: nodes = [] } = useQuery({
    queryKey: [nodesQueryKey],
    queryFn: getNodes,
  });

  const { data: bindings = [] } = useQuery({
    queryKey: [bindingsQueryKey],
    queryFn: getBindings,
  });

  const { data: protocols = [] } = useQuery({
    queryKey: [protocolsQueryKey],
    queryFn: getAllProtocols,
  });

  const { data: library = [] } = useQuery({
    queryKey: [libraryQueryKey],
    queryFn: getRuleSetLibrary,
  });

  const createMutation = useMutation({
    mutationFn: createRelayRule,
    onSuccess: () => {
      setCreateOpen(false);
      resetForm();
      queryClient.invalidateQueries({ queryKey: [relayRulesQueryKey] });
    },
  });

  const updateMutation = useMutation({
    mutationFn: (payload: { id: string; data: Parameters<typeof updateRelayRule>[1] }) =>
      updateRelayRule(payload.id, payload.data),
    onSuccess: () => {
      setEditRule(null);
      queryClient.invalidateQueries({ queryKey: [relayRulesQueryKey] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: deleteRelayRule,
    onSuccess: () => {
      setDeleteRuleId(null);
      queryClient.invalidateQueries({ queryKey: [relayRulesQueryKey] });
    },
  });

  const resetForm = (rule?: RelayRule) => {
    if (rule) {
      setForm(prefillForm(rule));
    } else {
      setForm(defaultForm);
    }
  };

  const handleCreate = () => {
    createMutation.mutate({
      name: form.name,
      node_id: form.node_id,
      exit_binding_id: form.exit_binding_id,
      match_type: form.match_type,
      match_config: buildMatchConfig(form),
      enabled: form.enabled,
      sort_order: Number(form.sort_order) || 0,
    });
  };

  const handleUpdate = () => {
    if (!editRule) return;
    updateMutation.mutate({
      id: editRule.id,
      data: {
        name: form.name,
        node_id: form.node_id,
        exit_binding_id: form.exit_binding_id,
        match_type: form.match_type,
        match_config: buildMatchConfig(form),
        enabled: form.enabled,
        sort_order: Number(form.sort_order) || 0,
      },
    });
  };

  const handleDelete = () => {
    if (!deleteRuleId) return;
    deleteMutation.mutate(deleteRuleId);
  };

  const openEdit = (rule: RelayRule) => {
    resetForm(rule);
    setEditRule(rule);
  };

  const getNodeName = (nodeId: string) => {
    const node = nodes.find((n) => n.id === nodeId);
    return node ? node.name : nodeId;
  };

  const getProtocolName = (protocolId: string) => {
    const protocol = protocols.find((p) => p.id === protocolId);
    return protocol ? protocol.name : protocolId;
  };

  // Filter active bindings whose node differs from the selected entry node
  const exitBindingOptions = bindings
    .filter((b) => b.is_active && b.node_id !== form.node_id)
    .map((b) => ({
      id: b.id,
      label: `${getNodeName(b.node_id)} / ${getProtocolName(b.protocol_config_id)}`,
    }));

  const libraryOptions = [
    ...library.map((e) => ({ id: e.name, label: e.name })),
    { id: "", label: t("relayRules.libraryCustom") },
  ];

  const matchSummary = (rule: RelayRule) => {
    if (rule.match_type === "inline") {
      const count =
        (rule.match_config.domains?.length || 0) + (rule.match_config.domain_suffixes?.length || 0);
      return t("relayRules.matchInline") + ` (${count})`;
    }
    if (rule.match_type === "rule_set") {
      if (rule.match_config.library) {
        return rule.match_config.library;
      }
      return t("relayRules.libraryCustom");
    }
    return "-";
  };

  const enabledBadge = (enabled: boolean) => {
    const color = enabled
      ? "bg-success-soft text-success-soft-foreground"
      : "bg-danger-soft text-danger-soft-foreground";
    return (
      <span
        className={`inline-flex items-center whitespace-nowrap rounded px-2 py-0.5 text-xs font-medium ${color}`}
      >
        {enabled ? t("common.enabled") : t("common.disabled")}
      </span>
    );
  };

  const loading = rulesLoading;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t("relayRules.title")}
        action={{
          label: t("relayRules.create"),
          onClick: () => {
            resetForm();
            setCreateOpen(true);
          },
        }}
      />

      <Card>
        <CardContent>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="relay rules">
              <TableHeader>
                <TableRow>
                  <TableHead>{t("common.name")}</TableHead>
                  <TableHead>{t("relayRules.entryNode")}</TableHead>
                  <TableHead>{t("relayRules.exitBinding")}</TableHead>
                  <TableHead>{t("relayRules.match")}</TableHead>
                  <TableHead>{t("common.enabled")}</TableHead>
                  <TableHead>{t("relayRules.sortOrder")}</TableHead>
                  <TableHead>{t("common.actions")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rules.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={7} className="p-4 text-center text-muted-foreground">
                      {t("relayRules.emptyHint")}
                    </TableCell>
                  </TableRow>
                )}
                {rules.map((rule) => (
                  <TableRow key={rule.id}>
                    <TableCell>{rule.name}</TableCell>
                    <TableCell>{rule.node_name || getNodeName(rule.node_id)}</TableCell>
                    <TableCell className="max-w-xs truncate">
                      {[rule.exit_node_name, rule.exit_config_name].filter(Boolean).join(" / ") ||
                        rule.exit_binding_id}
                    </TableCell>
                    <TableCell>{matchSummary(rule)}</TableCell>
                    <TableCell>{enabledBadge(rule.enabled)}</TableCell>
                    <TableCell>{rule.sort_order}</TableCell>
                    <TableCell>
                      <div className="flex gap-2">
                        <Button size="sm" variant="ghost" onClick={() => openEdit(rule)}>
                          {t("common.edit")}
                        </Button>
                        <Button
                          size="sm"
                          variant="destructive"
                          onClick={() => setDeleteRuleId(rule.id)}
                        >
                          {t("common.delete")}
                        </Button>
                      </div>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <ConfirmDialog
        title={t("relayRules.deleteTitle")}
        isOpen={!!deleteRuleId}
        onClose={() => setDeleteRuleId(null)}
        onConfirm={handleDelete}
      >
        {t("relayRules.deleteConfirm")}
      </ConfirmDialog>

      <RelayRuleDialog
        open={createOpen}
        title={t("relayRules.createTitle")}
        submitLabel={t("common.create")}
        form={form}
        nodeOptions={nodes.map((n) => ({ id: n.id, label: n.name }))}
        exitBindingOptions={exitBindingOptions}
        libraryOptions={libraryOptions}
        onFormChange={setForm}
        onClose={() => setCreateOpen(false)}
        onSubmit={handleCreate}
      />

      <RelayRuleDialog
        open={!!editRule}
        title={t("relayRules.editTitle")}
        submitLabel={t("common.update")}
        form={form}
        nodeOptions={nodes.map((n) => ({ id: n.id, label: n.name }))}
        exitBindingOptions={exitBindingOptions}
        libraryOptions={libraryOptions}
        onFormChange={setForm}
        onClose={() => setEditRule(null)}
        onSubmit={handleUpdate}
      />
    </div>
  );
}
