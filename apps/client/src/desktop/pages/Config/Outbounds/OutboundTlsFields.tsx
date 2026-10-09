import type { OutboundFormFields } from "@pp/client-core";
import { SectionTitle, SwitchRow, TextField } from "./OutboundField";

interface OutboundTlsFieldsProps {
  fields: OutboundFormFields;
  /** 局部字段补丁更新（父层持有完整草稿）。 */
  onChange: (patch: Partial<OutboundFormFields>) => void;
}

/**
 * 出站 TLS 区块（桌面端；语义对齐移动端 `OutboundTlsFields`）：
 * vless / vmess / trojan / hysteria2 共用。字段对齐 `OutboundTls`：
 * enabled / server_name / insecure / alpn（逗号分隔草稿）。
 */
export function OutboundTlsFields({ fields, onChange }: OutboundTlsFieldsProps) {
  return (
    <div className="flex flex-col gap-4">
      <SectionTitle>TLS</SectionTitle>
      <SwitchRow
        label="启用 TLS"
        ariaLabel="启用 TLS"
        isSelected={fields.tlsEnabled}
        onChange={(tlsEnabled) => onChange({ tlsEnabled })}
      />
      {fields.tlsEnabled && (
        <>
          <TextField
            id="outbound-tls-server-name"
            label="server_name (SNI)"
            value={fields.tlsServerName}
            onChange={(tlsServerName) => onChange({ tlsServerName })}
            placeholder="例如：example.com"
            hint="留空则使用服务器地址"
            mono
          />
          <SwitchRow
            label="跳过证书校验 (insecure)"
            ariaLabel="跳过证书校验"
            isSelected={fields.tlsInsecure}
            onChange={(tlsInsecure) => onChange({ tlsInsecure })}
          />
          <TextField
            id="outbound-tls-alpn"
            label="ALPN（可选）"
            value={fields.tlsAlpn}
            onChange={(tlsAlpn) => onChange({ tlsAlpn })}
            placeholder="例如：h2, http/1.1"
            hint="多个值用逗号分隔"
            mono
          />
        </>
      )}
    </div>
  );
}
