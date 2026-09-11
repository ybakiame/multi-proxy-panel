import type { OutboundFormErrors, OutboundFormFields } from "./outboundForm";
import { HYSTERIA2_OBFS_OPTIONS, SHADOWSOCKS_METHOD_OPTIONS, VMESS_SECURITY_OPTIONS } from "./outboundOptions";
import { SelectField, TextField } from "./OutboundField";

interface OutboundProtocolFieldsProps {
  fields: OutboundFormFields;
  errors: OutboundFormErrors;
  /** 局部字段补丁更新（父层持有完整草稿）。 */
  onChange: (patch: Partial<OutboundFormFields>) => void;
}

/**
 * 出站协议字段区（ADR-0005 P0-4c）：按当前协议条件渲染。
 *
 * 字段集严格对齐 client-core `OutboundProtocol` 各变体：server / server_port 通用；
 * vless（uuid/flow）、vmess（uuid/security/alter_id）、shadowsocks（method/password）、
 * trojan（password）、hysteria2（password/up_mbps/down_mbps/obfs）。
 */
export function OutboundProtocolFields({ fields, errors, onChange }: OutboundProtocolFieldsProps) {
  return (
    <>
      <TextField
        id="outbound-server"
        label="服务器地址"
        required
        value={fields.server}
        onChange={(server) => onChange({ server })}
        placeholder="例如：1.2.3.4 或 example.com"
        error={errors.server}
        hint="IP 或域名"
        mono
      />

      <TextField
        id="outbound-port"
        label="端口"
        required
        inputMode="numeric"
        value={fields.port}
        onChange={(port) => onChange({ port })}
        placeholder="1-65535"
        error={errors.port}
        hint="1-65535"
        mono
      />

      {fields.protocol === "vless" && (
        <>
          <TextField
            id="outbound-uuid"
            label="UUID"
            required
            value={fields.uuid}
            onChange={(uuid) => onChange({ uuid })}
            placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
            error={errors.uuid}
            hint="VLESS 用户 UUID"
            mono
          />
          <TextField
            id="outbound-flow"
            label="flow（可选）"
            value={fields.flow}
            onChange={(flow) => onChange({ flow })}
            placeholder="例如：xtls-rprx-vision"
            hint="留空为不使用"
            mono
          />
        </>
      )}

      {fields.protocol === "vmess" && (
        <>
          <TextField
            id="outbound-uuid"
            label="UUID"
            required
            value={fields.uuid}
            onChange={(uuid) => onChange({ uuid })}
            placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
            error={errors.uuid}
            hint="VMess 用户 UUID"
            mono
          />
          <SelectField
            label="加密方式"
            value={fields.security}
            onChange={(security) => onChange({ security })}
            options={VMESS_SECURITY_OPTIONS}
          />
          <TextField
            id="outbound-alter-id"
            label="alter id"
            inputMode="numeric"
            value={fields.alterId}
            onChange={(alterId) => onChange({ alterId })}
            placeholder="0"
            hint="0 表示 AEAD"
            mono
          />
        </>
      )}

      {fields.protocol === "shadowsocks" && (
        <>
          <SelectField
            label="加密方式"
            value={fields.method}
            onChange={(method) => onChange({ method })}
            options={SHADOWSOCKS_METHOD_OPTIONS}
            error={errors.method}
          />
          <TextField
            id="outbound-password"
            label="密码"
            required
            value={fields.password}
            onChange={(password) => onChange({ password })}
            error={errors.password}
            mono
          />
        </>
      )}

      {fields.protocol === "trojan" && (
        <TextField
          id="outbound-password"
          label="密码"
          required
          value={fields.password}
          onChange={(password) => onChange({ password })}
          error={errors.password}
          mono
        />
      )}

      {fields.protocol === "hysteria2" && (
        <>
          <TextField
            id="outbound-password"
            label="密码"
            required
            value={fields.password}
            onChange={(password) => onChange({ password })}
            error={errors.password}
            mono
          />
          <TextField
            id="outbound-up-mbps"
            label="上行带宽 Mbps（可选）"
            inputMode="numeric"
            value={fields.upMbps}
            onChange={(upMbps) => onChange({ upMbps })}
            placeholder="留空或 0 使用 BBR"
            hint="0 表示 BBR 拥塞控制"
            mono
          />
          <TextField
            id="outbound-down-mbps"
            label="下行带宽 Mbps（可选）"
            inputMode="numeric"
            value={fields.downMbps}
            onChange={(downMbps) => onChange({ downMbps })}
            placeholder="留空或 0 使用 BBR"
            hint="0 表示 BBR 拥塞控制"
            mono
          />
          <SelectField
            label="obfs 混淆（可选）"
            value={fields.obfsType}
            onChange={(obfsType) => onChange({ obfsType })}
            options={HYSTERIA2_OBFS_OPTIONS}
          />
          {fields.obfsType !== "" && (
            <TextField
              id="outbound-obfs-password"
              label="obfs 密码"
              value={fields.obfsPassword}
              onChange={(obfsPassword) => onChange({ obfsPassword })}
              hint="salamander / gecko 混淆密码"
              mono
            />
          )}
        </>
      )}
    </>
  );
}
