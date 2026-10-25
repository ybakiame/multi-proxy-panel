import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { FormCheckbox, FormInput, FormSelect, FormTextArea } from "../components/ui";

export interface RelayRuleForm {
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

interface SelectOption {
  id: string;
  label: string;
}

interface RelayRuleDialogProps {
  open: boolean;
  title: string;
  submitLabel: string;
  form: RelayRuleForm;
  nodeOptions: SelectOption[];
  exitBindingOptions: SelectOption[];
  libraryOptions: SelectOption[];
  onFormChange: (form: RelayRuleForm) => void;
  onClose: () => void;
  onSubmit: () => void;
}

export function RelayRuleDialog({
  open,
  title,
  submitLabel,
  form,
  nodeOptions,
  exitBindingOptions,
  libraryOptions,
  onFormChange,
  onClose,
  onSubmit,
}: RelayRuleDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (!nextOpen) onClose();
      }}
    >
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <FormInput
            label={t("common.name")}
            value={form.name}
            onChange={(value) => onFormChange({ ...form, name: value })}
            isRequired
          />
          <FormSelect
            label={t("relayRules.entryNode")}
            value={form.node_id}
            onChange={(value) => {
              onFormChange({ ...form, node_id: value, exit_binding_id: "" });
            }}
            options={nodeOptions}
            isRequired
          />
          <FormSelect
            label={t("relayRules.exitBinding")}
            value={form.exit_binding_id}
            onChange={(value) => onFormChange({ ...form, exit_binding_id: value })}
            options={exitBindingOptions}
            isRequired
            isDisabled={!form.node_id}
          />
          <FormSelect
            label={t("relayRules.match")}
            value={form.match_type}
            onChange={(value) =>
              onFormChange({
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
                onChange={(value) => onFormChange({ ...form, domains: value })}
                placeholder={t("relayRules.domainsHint")}
                rows={4}
              />
              <FormTextArea
                label={t("relayRules.domainSuffixes")}
                value={form.domain_suffixes}
                onChange={(value) => onFormChange({ ...form, domain_suffixes: value })}
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
                onChange={(value) => onFormChange({ ...form, library_name: value })}
                options={libraryOptions}
              />
              {!form.library_name && (
                <>
                  <FormInput
                    label={t("relayRules.customSingboxUrl")}
                    value={form.singbox_url}
                    onChange={(value) => onFormChange({ ...form, singbox_url: value })}
                    placeholder="https://..."
                  />
                  <FormInput
                    label={t("relayRules.customMihomoUrl")}
                    value={form.mihomo_url}
                    onChange={(value) => onFormChange({ ...form, mihomo_url: value })}
                    placeholder="https://..."
                  />
                </>
              )}
            </>
          )}
          <FormCheckbox
            isSelected={form.enabled}
            onChange={(selected) => onFormChange({ ...form, enabled: selected })}
          >
            {t("common.enabled")}
          </FormCheckbox>
          <FormInput
            label={t("relayRules.sortOrder")}
            value={form.sort_order}
            onChange={(value) => onFormChange({ ...form, sort_order: value })}
            type="number"
          />
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button
            disabled={!form.name.trim() || !form.node_id || !form.exit_binding_id}
            onClick={onSubmit}
          >
            {submitLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
