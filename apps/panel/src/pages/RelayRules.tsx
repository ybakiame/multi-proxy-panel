import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { Button, Card, Modal, Spinner, Table } from "@heroui/react";
import {
  PageHeader,
  ConfirmDialog,
  FormInput,
  FormSelect,
  FormTextArea,
  FormCheckbox,
} from "../components/ui";
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

interface RelayRuleForm {
  name: string;
  node_id: string;
  exit_binding_id: string;
  match_type: "inline" | "rule_set";
  domains: string;
  domain_suffixes: string;
  library_name: string;
  singbox_url: string;
  mihomo_url: string;
  enabled: boolean;
  sort_order: string;
}

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
        <Card.Content>
          {loading ? (
            <div className="flex h-32 items-center justify-center">
              <Spinner />
            </div>
          ) : (
            <Table aria-label="relay rules">
              <Table.ScrollContainer>
                <Table.Content>
                  <Table.Header>
                    <Table.Column isRowHeader>{t("common.name")}</Table.Column>
                    <Table.Column>{t("relayRules.entryNode")}</Table.Column>
                    <Table.Column>{t("relayRules.exitBinding")}</Table.Column>
                    <Table.Column>{t("relayRules.match")}</Table.Column>
                    <Table.Column>{t("common.enabled")}</Table.Column>
                    <Table.Column>{t("relayRules.sortOrder")}</Table.Column>
                    <Table.Column>{t("common.actions")}</Table.Column>
                  </Table.Header>
                  <Table.Body
                    renderEmptyState={() => (
                      <div className="p-4 text-center text-muted-foreground">
                        {t("relayRules.emptyHint")}
                      </div>
                    )}
                  >
                    {rules.map((rule) => (
                      <Table.Row key={rule.id}>
                        <Table.Cell>{rule.name}</Table.Cell>
                        <Table.Cell>{rule.node_name || getNodeName(rule.node_id)}</Table.Cell>
                        <Table.Cell className="max-w-xs truncate">
                          {[rule.exit_node_name, rule.exit_config_name]
                            .filter(Boolean)
                            .join(" / ") || rule.exit_binding_id}
                        </Table.Cell>
                        <Table.Cell>{matchSummary(rule)}</Table.Cell>
                        <Table.Cell>{enabledBadge(rule.enabled)}</Table.Cell>
                        <Table.Cell>{rule.sort_order}</Table.Cell>
                        <Table.Cell>
                          <div className="flex gap-2">
                            <Button size="sm" variant="ghost" onPress={() => openEdit(rule)}>
                              {t("common.edit")}
                            </Button>
                            <Button
                              size="sm"
                              variant="danger"
                              onPress={() => setDeleteRuleId(rule.id)}
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

      <ConfirmDialog
        title={t("relayRules.deleteTitle")}
        isOpen={!!deleteRuleId}
        onClose={() => setDeleteRuleId(null)}
        onConfirm={handleDelete}
      >
        {t("relayRules.deleteConfirm")}
      </ConfirmDialog>

      {/* Create Modal */}
      <Modal.Backdrop isOpen={createOpen} onOpenChange={(open) => setCreateOpen(open)}>
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("relayRules.createTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormSelect
                label={t("relayRules.entryNode")}
                value={form.node_id}
                onChange={(value) => {
                  setForm({ ...form, node_id: value, exit_binding_id: "" });
                }}
                options={nodes.map((n) => ({ id: n.id, label: n.name }))}
                isRequired
              />
              <FormSelect
                label={t("relayRules.exitBinding")}
                value={form.exit_binding_id}
                onChange={(value) => setForm({ ...form, exit_binding_id: value })}
                options={exitBindingOptions}
                isRequired
                isDisabled={!form.node_id}
              />
              <FormSelect
                label={t("relayRules.match")}
                value={form.match_type}
                onChange={(value) =>
                  setForm({
                    ...form,
                    match_type: value as "inline" | "rule_set",
                  })
                }
                options={[
                  { id: "inline", label: t("relayRules.matchInline") },
                  { id: "rule_set", label: t("relayRules.matchRuleSet") },
                ]}
                isRequired
              />
              {form.match_type === "inline" && (
                <>
                  <FormTextArea
                    label={t("relayRules.domains")}
                    value={form.domains}
                    onChange={(value) => setForm({ ...form, domains: value })}
                    placeholder={t("relayRules.domainsHint")}
                    rows={4}
                  />
                  <FormTextArea
                    label={t("relayRules.domainSuffixes")}
                    value={form.domain_suffixes}
                    onChange={(value) => setForm({ ...form, domain_suffixes: value })}
                    placeholder={t("relayRules.domainsHint")}
                    rows={4}
                  />
                </>
              )}
              {form.match_type === "rule_set" && (
                <>
                  <FormSelect
                    label={t("relayRules.library")}
                    value={form.library_name}
                    onChange={(value) => setForm({ ...form, library_name: value })}
                    options={libraryOptions}
                  />
                  {!form.library_name && (
                    <>
                      <FormInput
                        label={t("relayRules.customSingboxUrl")}
                        value={form.singbox_url}
                        onChange={(value) => setForm({ ...form, singbox_url: value })}
                        placeholder="https://..."
                      />
                      <FormInput
                        label={t("relayRules.customMihomoUrl")}
                        value={form.mihomo_url}
                        onChange={(value) => setForm({ ...form, mihomo_url: value })}
                        placeholder="https://..."
                      />
                    </>
                  )}
                </>
              )}
              <FormCheckbox
                isSelected={form.enabled}
                onChange={(selected) => setForm({ ...form, enabled: selected })}
              >
                {t("common.enabled")}
              </FormCheckbox>
              <FormInput
                label={t("relayRules.sortOrder")}
                value={form.sort_order}
                onChange={(value) => setForm({ ...form, sort_order: value })}
                type="number"
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setCreateOpen(false)}>
                {t("common.cancel")}
              </Button>
              <Button
                isDisabled={!form.name.trim() || !form.node_id || !form.exit_binding_id}
                onPress={handleCreate}
              >
                {t("common.create")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>

      {/* Edit Modal */}
      <Modal.Backdrop
        isOpen={!!editRule}
        onOpenChange={(open) => {
          if (!open) setEditRule(null);
        }}
      >
        <Modal.Container>
          <Modal.Dialog>
            <Modal.Header>
              <Modal.Heading>{t("relayRules.editTitle")}</Modal.Heading>
            </Modal.Header>
            <Modal.Body className="space-y-4">
              <FormInput
                label={t("common.name")}
                value={form.name}
                onChange={(value) => setForm({ ...form, name: value })}
                isRequired
              />
              <FormSelect
                label={t("relayRules.entryNode")}
                value={form.node_id}
                onChange={(value) => {
                  setForm({ ...form, node_id: value, exit_binding_id: "" });
                }}
                options={nodes.map((n) => ({ id: n.id, label: n.name }))}
                isRequired
              />
              <FormSelect
                label={t("relayRules.exitBinding")}
                value={form.exit_binding_id}
                onChange={(value) => setForm({ ...form, exit_binding_id: value })}
                options={exitBindingOptions}
                isRequired
                isDisabled={!form.node_id}
              />
              <FormSelect
                label={t("relayRules.match")}
                value={form.match_type}
                onChange={(value) =>
                  setForm({
                    ...form,
                    match_type: value as "inline" | "rule_set",
                  })
                }
                options={[
                  { id: "inline", label: t("relayRules.matchInline") },
                  { id: "rule_set", label: t("relayRules.matchRuleSet") },
                ]}
                isRequired
              />
              {form.match_type === "inline" && (
                <>
                  <FormTextArea
                    label={t("relayRules.domains")}
                    value={form.domains}
                    onChange={(value) => setForm({ ...form, domains: value })}
                    placeholder={t("relayRules.domainsHint")}
                    rows={4}
                  />
                  <FormTextArea
                    label={t("relayRules.domainSuffixes")}
                    value={form.domain_suffixes}
                    onChange={(value) => setForm({ ...form, domain_suffixes: value })}
                    placeholder={t("relayRules.domainsHint")}
                    rows={4}
                  />
                </>
              )}
              {form.match_type === "rule_set" && (
                <>
                  <FormSelect
                    label={t("relayRules.library")}
                    value={form.library_name}
                    onChange={(value) => setForm({ ...form, library_name: value })}
                    options={libraryOptions}
                  />
                  {!form.library_name && (
                    <>
                      <FormInput
                        label={t("relayRules.customSingboxUrl")}
                        value={form.singbox_url}
                        onChange={(value) => setForm({ ...form, singbox_url: value })}
                        placeholder="https://..."
                      />
                      <FormInput
                        label={t("relayRules.customMihomoUrl")}
                        value={form.mihomo_url}
                        onChange={(value) => setForm({ ...form, mihomo_url: value })}
                        placeholder="https://..."
                      />
                    </>
                  )}
                </>
              )}
              <FormCheckbox
                isSelected={form.enabled}
                onChange={(selected) => setForm({ ...form, enabled: selected })}
              >
                {t("common.enabled")}
              </FormCheckbox>
              <FormInput
                label={t("relayRules.sortOrder")}
                value={form.sort_order}
                onChange={(value) => setForm({ ...form, sort_order: value })}
                type="number"
              />
            </Modal.Body>
            <Modal.Footer>
              <Button slot="close" variant="ghost" onPress={() => setEditRule(null)}>
                {t("common.cancel")}
              </Button>
              <Button
                isDisabled={!form.name.trim() || !form.node_id || !form.exit_binding_id}
                onPress={handleUpdate}
              >
                {t("common.update")}
              </Button>
            </Modal.Footer>
          </Modal.Dialog>
        </Modal.Container>
      </Modal.Backdrop>
    </div>
  );
}
