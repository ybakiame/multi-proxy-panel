import type { OutboundFormFields } from "./outboundForm";
import {
  OUTBOUND_TRANSPORT_OPTIONS,
  transportPathLabel,
  transportUsesHost,
  transportUsesPath,
} from "./outboundOptions";
import { SectionTitle, SelectField, TextField } from "./OutboundField";

interface OutboundTransportFieldsProps {
  fields: OutboundFormFields;
  /** 局部字段补丁更新（父层持有完整草稿）。 */
  onChange: (patch: Partial<OutboundFormFields>) => void;
}

/**
 * 出站传输区块（ADR-0005 P0-4c）：vless / vmess / trojan 共用。
 *
 * 字段对齐 `OutboundTransport`：kind / path / host。按 kind 条件展示：
 * tcp 无附加字段；gRPC 的 path 语义为 service_name 且无 host；其余展示 path（+host）。
 */
export function OutboundTransportFields({ fields, onChange }: OutboundTransportFieldsProps) {
  const kind = fields.transportKind;
  return (
    <div className="flex flex-col gap-4">
      <SectionTitle>传输方式</SectionTitle>
      <SelectField
        label="传输类型"
        value={kind}
        onChange={(transportKind) => onChange({ transportKind })}
        options={OUTBOUND_TRANSPORT_OPTIONS}
      />
      {transportUsesPath(kind) && (
        <TextField
          id="outbound-transport-path"
          label={transportPathLabel(kind)}
          value={fields.transportPath}
          onChange={(transportPath) => onChange({ transportPath })}
          placeholder={kind === "grpc" ? "例如：GunService" : "例如：/path"}
          mono
        />
      )}
      {transportUsesHost(kind) && (
        <TextField
          id="outbound-transport-host"
          label="Host"
          value={fields.transportHost}
          onChange={(transportHost) => onChange({ transportHost })}
          placeholder="例如：example.com"
          mono
        />
      )}
    </div>
  );
}
