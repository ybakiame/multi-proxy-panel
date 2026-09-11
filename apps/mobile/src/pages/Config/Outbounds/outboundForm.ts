import { isGroupOutbound, outboundTag } from "@pp/client-core";
import type {
  ConfigSlices,
  CustomOutbound,
  GroupOutbound,
  OutboundTls,
  OutboundTransport,
  OutboundsSlice,
} from "@pp/client-core";
import { applyGroupToForm, groupFormToOutbound, validateGroupItem } from "./groupForm";
import { isGroupProtocol, type OutboundProtocolType } from "./outboundOptions";

/**
 * 自定义出站切片（ADR-0005 P0-4c）表单映射与校验。
 *
 * 校验对齐 ADR §3.5（D5 前端表单即时校验）与 Rust `OutboundsSlice::validate`：名称必填、
 * tag（`slice-` slug）唯一、server 必填、端口 1-65535；前端额外要求协议必填字段
 * （uuid / password / method）。表单采用扁平字符串草稿（数字字段存字符串），保存时经
 * `formToOutbound` 转回 `CustomOutbound`，对齐 `DnsServerFormSheet` 的
 * 「字符串草稿 + 保存转换」模式。
 */

// ---------------------------------------------------------------------------
// 结构守卫
// ---------------------------------------------------------------------------

/**
 * `ConfigSlices` 结构守卫（出站切片视角）：`CONFIG_SLICES_KEY` 缓存形态异常时
 * 视为未加载，页面渲染空态而非访问 undefined 崩溃（对齐 dnsUtils 的第二道防线）。
 */
export function isConfigSlices(value: unknown): value is ConfigSlices {
  if (!value || typeof value !== "object") {
    return false;
  }
  const slices = value as ConfigSlices;
  return typeof slices.outbounds === "object" && slices.outbounds !== null && Array.isArray(slices.outbounds.items);
}

// ---------------------------------------------------------------------------
// 表单草稿 <-> 结构化出站
// ---------------------------------------------------------------------------

/** 出站编辑表单草稿（数字字段以字符串承载，保存时转换）。 */
export interface OutboundFormFields {
  name: string;
  enabled: boolean;
  protocol: OutboundProtocolType;
  server: string;
  port: string;
  uuid: string;
  flow: string;
  security: string;
  alterId: string;
  method: string;
  password: string;
  upMbps: string;
  downMbps: string;
  obfsType: string;
  obfsPassword: string;
  tlsEnabled: boolean;
  tlsServerName: string;
  tlsInsecure: boolean;
  tlsAlpn: string;
  transportKind: string;
  transportPath: string;
  transportHost: string;
  /** 分组（selector / urltest）成员 tag 列表。 */
  members: string[];
  /** selector 默认成员 tag（空 = 使用第一个成员）。 */
  groupDefault: string;
  /** 分组切换成员时是否中断现有连接。 */
  interruptExistConnections: boolean;
  /** urltest 测速 URL（空 = 核心默认）。 */
  groupUrl: string;
  /** urltest 测速间隔（空 = 核心默认，如 `3m`）。 */
  groupInterval: string;
  /** urltest 容差毫秒（`0` = 核心默认）。 */
  groupTolerance: string;
}

/** 协议默认端口。 */
function defaultPort(type: OutboundProtocolType): string {
  return type === "shadowsocks" ? "8388" : "443";
}

/** 协议默认是否启用 TLS（trojan / hysteria2 协议本身依赖 TLS）。 */
function defaultTlsEnabled(type: OutboundProtocolType): boolean {
  return type === "trojan" || type === "hysteria2";
}

/** 生成指定协议的默认表单草稿（切换协议时重置协议字段用）。 */
export function defaultOutboundForm(type: OutboundProtocolType = "vless"): OutboundFormFields {
  return {
    name: "",
    enabled: true,
    protocol: type,
    server: "",
    port: defaultPort(type),
    uuid: "",
    flow: "",
    security: "auto",
    alterId: "0",
    method: "2022-blake3-aes-128-gcm",
    password: "",
    upMbps: "",
    downMbps: "",
    obfsType: "",
    obfsPassword: "",
    tlsEnabled: defaultTlsEnabled(type),
    tlsServerName: "",
    tlsInsecure: false,
    tlsAlpn: "",
    transportKind: "tcp",
    transportPath: "",
    transportHost: "",
    members: [],
    groupDefault: "",
    interruptExistConnections: false,
    groupUrl: "",
    groupInterval: "",
    groupTolerance: "0",
  };
}

/** 结构化出站 → 表单草稿（编辑预填）。 */
export function outboundToForm(item: CustomOutbound): OutboundFormFields {
  const form = defaultOutboundForm(item.type);
  form.name = item.name;
  form.enabled = item.enabled;
  if (isGroupOutbound(item)) {
    applyGroupToForm(form, item);
    return form;
  }
  form.server = item.server;
  form.port = item.server_port > 0 ? String(item.server_port) : "";
  switch (item.type) {
    case "vless":
      form.uuid = item.uuid;
      form.flow = item.flow;
      applyTlsToForm(form, item.tls);
      applyTransportToForm(form, item.transport);
      break;
    case "vmess":
      form.uuid = item.uuid;
      form.security = item.security || "auto";
      form.alterId = String(item.alter_id);
      applyTlsToForm(form, item.tls);
      applyTransportToForm(form, item.transport);
      break;
    case "shadowsocks":
      form.method = item.method || form.method;
      form.password = item.password;
      break;
    case "trojan":
      form.password = item.password;
      applyTlsToForm(form, item.tls);
      applyTransportToForm(form, item.transport);
      break;
    case "hysteria2":
      form.password = item.password;
      form.upMbps = item.up_mbps > 0 ? String(item.up_mbps) : "";
      form.downMbps = item.down_mbps > 0 ? String(item.down_mbps) : "";
      form.obfsType = item.obfs.obfs_type;
      form.obfsPassword = item.obfs.password;
      applyTlsToForm(form, item.tls);
      break;
  }
  return form;
}

function applyTlsToForm(form: OutboundFormFields, tls: OutboundTls): void {
  form.tlsEnabled = tls.enabled;
  form.tlsServerName = tls.server_name;
  form.tlsInsecure = tls.insecure;
  form.tlsAlpn = tls.alpn.join(", ");
}

function applyTransportToForm(form: OutboundFormFields, transport: OutboundTransport): void {
  form.transportKind = transport.kind || "tcp";
  form.transportPath = transport.path;
  form.transportHost = transport.host;
}

/** 表单草稿 → 结构化出站（保存转换）。 */
export function formToOutbound(fields: OutboundFormFields, id: string): CustomOutbound {
  const base = { id, name: fields.name.trim(), enabled: fields.enabled };
  if (isGroupProtocol(fields.protocol)) {
    return groupFormToOutbound(fields, base);
  }
  const server = fields.server.trim();
  const server_port = parsePortValue(fields.port);
  const tls = buildTls(fields);
  const transport = buildTransport(fields);
  switch (fields.protocol) {
    case "vless":
      return {
        ...base,
        type: "vless",
        server,
        server_port,
        uuid: fields.uuid.trim(),
        flow: fields.flow.trim(),
        tls,
        transport,
      };
    case "vmess":
      return {
        ...base,
        type: "vmess",
        server,
        server_port,
        uuid: fields.uuid.trim(),
        security: fields.security,
        alter_id: parseUint(fields.alterId),
        tls,
        transport,
      };
    case "shadowsocks":
      return { ...base, type: "shadowsocks", server, server_port, method: fields.method, password: fields.password };
    case "trojan":
      return { ...base, type: "trojan", server, server_port, password: fields.password, tls, transport };
    case "hysteria2":
      return {
        ...base,
        type: "hysteria2",
        server,
        server_port,
        password: fields.password,
        up_mbps: parseUint(fields.upMbps),
        down_mbps: parseUint(fields.downMbps),
        obfs: { obfs_type: fields.obfsType, password: fields.obfsPassword },
        tls,
      };
  }
}

function buildTls(fields: OutboundFormFields): OutboundTls {
  return {
    enabled: fields.tlsEnabled,
    server_name: fields.tlsServerName.trim(),
    insecure: fields.tlsInsecure,
    alpn: parseAlpn(fields.tlsAlpn),
  };
}

function buildTransport(fields: OutboundFormFields): OutboundTransport {
  return {
    kind: fields.transportKind,
    path: fields.transportPath.trim(),
    host: fields.transportHost.trim(),
  };
}

/** ALPN 草稿（逗号/空白分隔）→ 字符串数组。 */
export function parseAlpn(raw: string): string[] {
  return raw
    .split(/[\s,]+/)
    .map((value) => value.trim())
    .filter((value) => value !== "");
}

/** 非负整数草稿解析（非法 → 0）。 */
function parseUint(raw: string): number {
  const value = Number(raw.trim());
  return Number.isFinite(value) && value >= 0 ? Math.trunc(value) : 0;
}

/** 端口草稿解析（非法 → 0；保存前已由校验保证合法）。 */
function parsePortValue(raw: string): number {
  const value = Number(raw.trim());
  return Number.isInteger(value) && value >= 1 && value <= 65535 ? value : 0;
}

// ---------------------------------------------------------------------------
// 标签 / 摘要
// ---------------------------------------------------------------------------

/** 节点出站（非分组）类型：`outboundSummary` 仅接受节点。 */
export type NodeOutbound = Exclude<CustomOutbound, GroupOutbound>;

/** 节点出站摘要（列表卡片副标题）：server:port。 */
export function outboundSummary(item: NodeOutbound): string {
  return `${item.server}:${item.server_port}`;
}

// ---------------------------------------------------------------------------
// 校验
// ---------------------------------------------------------------------------

const UUID_PATTERN = /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

/** UUID 格式校验（vless / vmess）。 */
export function isUuid(value: string): boolean {
  return UUID_PATTERN.test(value.trim());
}

/** 端口字段校验（必填、1-65535）。 */
export function validatePortField(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed === "") {
    return "请输入端口";
  }
  if (!/^\d+$/.test(trimmed)) {
    return "端口需为 1-65535 的整数";
  }
  const value = Number(trimmed);
  if (value < 1 || value > 65535) {
    return "端口需在 1-65535 之间";
  }
  return null;
}

/** 出站表单字段错误（`null` = 合法）。 */
export interface OutboundFormErrors {
  name: string | null;
  server: string | null;
  port: string | null;
  uuid: string | null;
  method: string | null;
  password: string | null;
}

/**
 * 校验出站表单草稿。`otherNames` 为除自身外的已有出站名称（编辑时排除自身），
 * 用于 tag（`slice-` slug）唯一性校验。
 */
export function validateOutboundForm(fields: OutboundFormFields, otherNames: readonly string[]): OutboundFormErrors {
  const name = fields.name.trim();
  let nameError: string | null = null;
  if (name === "") {
    nameError = "请输入名称";
  } else {
    const tag = outboundTag(name);
    if (otherNames.some((other) => outboundTag(other) === tag)) {
      nameError = "名称生成的 tag 与已有出站冲突，请修改名称";
    }
  }

  const isGroup = isGroupProtocol(fields.protocol);
  const serverError = isGroup || fields.server.trim() !== "" ? null : "请输入服务器地址";
  const portError = isGroup ? null : validatePortField(fields.port);

  const needsUuid = fields.protocol === "vless" || fields.protocol === "vmess";
  let uuidError: string | null = null;
  if (needsUuid) {
    if (fields.uuid.trim() === "") {
      uuidError = "请输入 UUID";
    } else if (!isUuid(fields.uuid)) {
      uuidError = "UUID 格式不正确";
    }
  }

  const needsPassword =
    fields.protocol === "shadowsocks" || fields.protocol === "trojan" || fields.protocol === "hysteria2";
  const passwordError = needsPassword && fields.password.trim() === "" ? "请输入密码" : null;
  const methodError = fields.protocol === "shadowsocks" && fields.method.trim() === "" ? "请选择加密方式" : null;

  return {
    name: nameError,
    server: serverError,
    port: portError,
    uuid: uuidError,
    method: methodError,
    password: passwordError,
  };
}

/** 表单校验结果是否全部通过。 */
export function isOutboundFormValid(errors: OutboundFormErrors): boolean {
  return (
    errors.name === null &&
    errors.server === null &&
    errors.port === null &&
    errors.uuid === null &&
    errors.method === null &&
    errors.password === null
  );
}

/** 整切片校验结果（页面保存按钮据此禁用）。 */
export interface OutboundsSliceErrors {
  /** 与 `outbounds.items` 等长，逐项首个错误（`null` = 合法）。 */
  itemErrors: (string | null)[];
}

/**
 * 校验整个自定义出站切片草稿（保存前调用）。
 *
 * 校验所有条目（含 disabled，对齐 Dns 页的宽松策略）；跨条目检测 tag 重复，
 * 分组条目额外校验成员非空、selector default ∈ 成员，以及成员引用的切片节点
 * 是否仍存在且启用（悬空引用行内提示，对齐 Rust `validate_group_members`）。
 */
export function validateOutboundsSlice(slice: OutboundsSlice): OutboundsSliceErrors {
  const enabledNodeTags = new Set<string>();
  for (const item of slice.items) {
    if (item.enabled && !isGroupOutbound(item)) {
      enabledNodeTags.add(outboundTag(item.name));
    }
  }

  const tags = new Set<string>();
  const itemErrors = slice.items.map((item) => {
    if (item.name.trim() === "") {
      return "名称不能为空";
    }
    const tag = outboundTag(item.name);
    if (tags.has(tag)) {
      return `tag「${tag}」与其它出站重复`;
    }
    tags.add(tag);
    if (isGroupOutbound(item)) {
      return validateGroupItem(item, enabledNodeTags);
    }
    if (item.server.trim() === "") {
      return "服务器地址不能为空";
    }
    if (!Number.isInteger(item.server_port) || item.server_port < 1 || item.server_port > 65535) {
      return "端口需在 1-65535 之间";
    }
    return null;
  });
  return { itemErrors };
}

/** 整切片校验结果是否全部通过。 */
export function isOutboundsSliceValid(errors: OutboundsSliceErrors): boolean {
  return errors.itemErrors.every((error) => error === null);
}
