import { useTranslation } from "react-i18next";
import { FormCheckbox, FormInput, FormSelect } from "../components/ui";
import { Button } from "../components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../components/ui/dialog";
import { ProtocolConfig } from "../api/types";

const PROTOCOL_TYPES = ["vless_reality", "vless_xhttp", "hysteria2", "anytls"];

// Mirrors CoreType::valid_for() in crates/pp-common/src/protocol.rs
const CORE_PROTOCOLS: Record<string, string[]> = {
  "sing-box": ["vless_reality", "hysteria2", "anytls"],
  mihomo: ["vless_reality", "vless_xhttp", "hysteria2", "anytls"],
};

const CORE_TYPES = ["sing-box", "mihomo"];
const FLOW_OPTIONS = ["xtls-rprx-vision"];
const XHTTP_MODES = ["auto", "packet-up", "stream-up"];
const OBFS_TYPES = ["none", "salamander"];

export interface ProtocolForm {
  name: string;
  protocol_type: string;
  core_type: string;
  listen_address: string;
  listen_port: string;
  tls_enabled: boolean;
  uuid: string;
  password: string;
  flow: string;
  dest: string;
  server_names: string;
  private_key: string;
  public_key: string;
  short_id: string;
  path: string;
  host: string;
  mode: string;
  obfs_type: string;
  obfs_password: string;
  up_mbps: string;
  down_mbps: string;
  masquerade: string;
}

interface ProtocolFormDialogProps {
  isOpen: boolean;
  onOpenChange: (open: boolean) => void;
  editingProtocol: ProtocolConfig | null;
  form: ProtocolForm;
  onFormChange: (form: ProtocolForm) => void;
  onSubmit: () => void;
  onGenerateRealityKeys: () => void;
  onGenerateRandomPort: () => void;
}

export function ProtocolFormDialog({
  isOpen,
  onOpenChange,
  editingProtocol,
  form,
  onFormChange,
  onSubmit,
  onGenerateRealityKeys,
  onGenerateRandomPort,
}: ProtocolFormDialogProps) {
  const { t } = useTranslation();

  const renderDynamicFields = () => {
    switch (form.protocol_type) {
      case "vless_reality":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => onFormChange({ ...form, uuid: value })}
              description={t("protocols.uuidDescription")}
              isRequired
            />
            <FormSelect
              label={t("protocols.flow")}
              value={form.flow}
              onChange={(value) => onFormChange({ ...form, flow: value })}
              options={FLOW_OPTIONS.map((option) => ({
                id: option,
                label: option,
              }))}
            />
            <FormInput
              label={t("protocols.dest")}
              value={form.dest}
              onChange={(value) => onFormChange({ ...form, dest: value })}
              isRequired
            />
            <FormInput
              label={t("protocols.serverNames")}
              value={form.server_names}
              onChange={(value) => onFormChange({ ...form, server_names: value })}
              placeholder="example.com, example.net"
              isRequired
            />
            <div className="flex gap-2">
              <FormInput
                className="flex-1"
                label={t("protocols.privateKey")}
                value={form.private_key}
                onChange={(value) => onFormChange({ ...form, private_key: value })}
              />
              <Button className="self-end" variant="ghost" onClick={onGenerateRealityKeys}>
                {t("protocols.generateKeys")}
              </Button>
            </div>
            <FormInput
              label={t("protocols.publicKey")}
              value={form.public_key}
              onChange={(value) => onFormChange({ ...form, public_key: value })}
            />
            <FormInput
              label={t("protocols.shortId")}
              value={form.short_id}
              onChange={(value) => onFormChange({ ...form, short_id: value })}
            />
          </>
        );
      case "vless_xhttp":
        return (
          <>
            <FormInput
              label={t("protocols.uuid")}
              value={form.uuid}
              onChange={(value) => onFormChange({ ...form, uuid: value })}
              description={t("protocols.uuidDescription")}
              isRequired
            />
            <FormInput
              label={t("protocols.path")}
              value={form.path}
              onChange={(value) => onFormChange({ ...form, path: value })}
            />
            <FormInput
              label={t("protocols.host")}
              value={form.host}
              onChange={(value) => onFormChange({ ...form, host: value })}
            />
            <FormSelect
              label={t("protocols.mode")}
              value={form.mode}
              onChange={(value) => onFormChange({ ...form, mode: value })}
              options={XHTTP_MODES.map((option) => ({
                id: option,
                label: option,
              }))}
            />
          </>
        );
      case "hysteria2":
        return (
          <>
            <FormInput
              label={t("protocols.password")}
              value={form.password}
              onChange={(value) => onFormChange({ ...form, password: value })}
              description={t("protocols.passwordDescription")}
              isRequired
            />
            <FormSelect
              label={t("protocols.obfsType")}
              value={form.obfs_type}
              onChange={(value) => onFormChange({ ...form, obfs_type: value })}
              options={OBFS_TYPES.map((option) => ({
                id: option,
                label: option,
              }))}
            />
            <FormInput
              label={t("protocols.obfsPassword")}
              value={form.obfs_password}
              onChange={(value) => onFormChange({ ...form, obfs_password: value })}
            />
            <FormInput
              type="number"
              label={t("protocols.upMbps")}
              value={form.up_mbps}
              onChange={(value) => onFormChange({ ...form, up_mbps: value })}
            />
            <FormInput
              type="number"
              label={t("protocols.downMbps")}
              value={form.down_mbps}
              onChange={(value) => onFormChange({ ...form, down_mbps: value })}
            />
          </>
        );
      case "anytls":
        return (
          <>
            <FormInput
              label={t("protocols.password")}
              value={form.password}
              onChange={(value) => onFormChange({ ...form, password: value })}
              description={t("protocols.passwordDescription")}
              isRequired
            />
            <FormInput
              label={t("protocols.masquerade")}
              value={form.masquerade}
              onChange={(value) => onFormChange({ ...form, masquerade: value })}
            />
          </>
        );
      default:
        return null;
    }
  };

  return (
    <Dialog open={isOpen} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {editingProtocol ? t("protocols.editTitle") : t("protocols.createTitle")}
          </DialogTitle>
        </DialogHeader>
        <div className="space-y-4">
          <FormInput
            label={t("common.name")}
            value={form.name}
            onChange={(value) => onFormChange({ ...form, name: value })}
            isRequired
          />
          <FormSelect
            label={t("protocols.core")}
            value={form.core_type}
            onChange={(value) => {
              const supported = CORE_PROTOCOLS[value] ?? PROTOCOL_TYPES;
              onFormChange({
                ...form,
                core_type: value,
                protocol_type: supported.includes(form.protocol_type)
                  ? form.protocol_type
                  : supported[0],
              });
            }}
            options={CORE_TYPES.map((core) => ({ id: core, label: core }))}
            isRequired
          />
          <FormSelect
            label={t("protocols.type")}
            value={form.protocol_type}
            onChange={(value) => onFormChange({ ...form, protocol_type: value })}
            options={(CORE_PROTOCOLS[form.core_type] ?? PROTOCOL_TYPES).map((type) => ({
              id: type,
              label: type,
            }))}
            isRequired
          />
          <FormInput
            label={t("protocols.listen")}
            value={form.listen_address}
            onChange={(value) => onFormChange({ ...form, listen_address: value })}
            isRequired
          />
          <div className="flex gap-2">
            <FormInput
              className="flex-1"
              type="number"
              label={t("protocols.port")}
              value={form.listen_port}
              onChange={(value) => onFormChange({ ...form, listen_port: value })}
              isRequired
            />
            <Button className="self-end" variant="ghost" onClick={onGenerateRandomPort}>
              {t("protocols.randomPort")}
            </Button>
          </div>
          {form.protocol_type !== "vless_reality" && (
            <FormCheckbox
              isSelected={form.tls_enabled}
              onChange={(selected) => onFormChange({ ...form, tls_enabled: selected })}
            >
              {t("protocols.useTls")}
            </FormCheckbox>
          )}
          {renderDynamicFields()}
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={onSubmit}>
            {editingProtocol ? t("common.update") : t("common.create")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
