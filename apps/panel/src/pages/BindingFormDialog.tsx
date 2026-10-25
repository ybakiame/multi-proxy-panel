import { useTranslation } from "react-i18next";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { FormCheckbox, FormInput, FormSelect, JsonEditor } from "../components/ui";
import { ManagedCertificate, Node, ProtocolConfig } from "../api/types";

export interface BindingForm {
  node_id: string;
  protocol_config_id: string;
  is_active: boolean;
  override_settings: string;
  tls_type: "none" | "certificate" | "acme" | "managed";
  tls_cert_file: string;
  tls_key_file: string;
  tls_domain: string;
  tls_cert_id: string;
}

export const defaultForm: BindingForm = {
  node_id: "",
  protocol_config_id: "",
  is_active: true,
  override_settings: "{}",
  tls_type: "none",
  tls_cert_file: "",
  tls_key_file: "",
  tls_domain: "",
  tls_cert_id: "",
};

interface BindingFormDialogProps {
  mode: "create" | "edit";
  open: boolean;
  form: BindingForm;
  nodes: Node[];
  protocols: ProtocolConfig[];
  certificates: ManagedCertificate[];
  onOpenChange: (open: boolean) => void;
  onFormChange: (updates: Partial<BindingForm>) => void;
  onOverrideSettingsChange: (value: string) => void;
  onSubmit: () => void;
}

export function BindingFormDialog({
  mode,
  open,
  form,
  nodes,
  protocols,
  certificates,
  onOpenChange,
  onFormChange,
  onOverrideSettingsChange,
  onSubmit,
}: BindingFormDialogProps) {
  const { t } = useTranslation();
  const selectedProtocol = protocols.find((p) => p.id === form.protocol_config_id);
  const showTlsOverride = !!selectedProtocol && selectedProtocol.protocol_type !== "vless_reality";
  const isSingBox = selectedProtocol?.core_type === "sing-box";
  const isEdit = mode === "edit";

  const renderTlsFields = () => {
    if (!showTlsOverride) return null;
    const tlsOptions = [
      { id: "none", label: t("protocols.tlsTypeNone") },
      { id: "managed", label: t("protocols.tlsTypeManaged") },
      { id: "certificate", label: t("protocols.tlsTypeCertificate") },
      ...(isSingBox ? [{ id: "acme", label: t("protocols.tlsTypeAcme") }] : []),
    ];
    return (
      <div className="space-y-4 rounded-lg border border-border p-4">
        <div className="text-sm font-medium">{t("bindings.tlsOverride")}</div>
        <FormSelect
          label={t("protocols.tlsType")}
          value={form.tls_type}
          onChange={(value) => onFormChange({ tls_type: value as BindingForm["tls_type"] })}
          options={tlsOptions}
        />
        {form.tls_type === "managed" && (
          <FormSelect
            label={t("protocols.managedCert")}
            value={form.tls_cert_id}
            onChange={(value) => onFormChange({ tls_cert_id: value })}
            options={certificates
              .filter((c) => c.status === "active" && c.node_id === form.node_id)
              .map((c) => ({ id: c.id, label: c.domain }))}
            isRequired
          />
        )}
        {form.tls_type === "certificate" && (
          <>
            <FormInput
              label={t("protocols.tlsCertFile")}
              value={form.tls_cert_file}
              onChange={(value) => onFormChange({ tls_cert_file: value })}
              placeholder="/etc/ssl/certs/example.crt"
              isRequired
            />
            <FormInput
              label={t("protocols.tlsKeyFile")}
              value={form.tls_key_file}
              onChange={(value) => onFormChange({ tls_key_file: value })}
              placeholder="/etc/ssl/private/example.key"
              isRequired
            />
          </>
        )}
        {form.tls_type === "acme" && isSingBox && (
          <FormInput
            label={t("protocols.tlsDomain")}
            value={form.tls_domain}
            onChange={(value) => onFormChange({ tls_domain: value })}
            placeholder="hy2.example.com"
            description={t("protocols.tlsDomainDescription")}
            isRequired
          />
        )}
      </div>
    );
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{t(isEdit ? "bindings.editTitle" : "bindings.createTitle")}</DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <FormSelect
            label={t("bindings.node")}
            value={form.node_id}
            onChange={(value) => onFormChange({ node_id: value })}
            options={nodes.map((node) => ({
              id: node.id,
              label: node.name,
            }))}
            isRequired={!isEdit}
            isDisabled={isEdit}
          />
          <FormSelect
            label={t("bindings.protocol")}
            value={form.protocol_config_id}
            onChange={(value) => onFormChange({ protocol_config_id: value })}
            options={protocols.map((protocol) => ({
              id: protocol.id,
              label: protocol.name,
            }))}
            isRequired={!isEdit}
            isDisabled={isEdit}
          />
          <FormCheckbox
            isSelected={form.is_active}
            onChange={(selected) => onFormChange({ is_active: selected })}
          >
            {t("common.active")}
          </FormCheckbox>
          {renderTlsFields()}
          <JsonEditor
            label={t("bindings.overrideSettings")}
            value={form.override_settings}
            onChange={onOverrideSettingsChange}
          />
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={onSubmit}>{t(isEdit ? "common.update" : "common.create")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
