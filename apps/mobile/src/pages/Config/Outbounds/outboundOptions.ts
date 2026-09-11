import type { OutboundProtocol } from "@pp/client-core";

/**
 * 自定义出站切片（ADR-0005 P0-4c）枚举选项与展示标签：协议 / 传输 / 加密方式，
 * 以及传输字段的可见性判定。纯常量与纯函数，无状态。
 */

/** 协议类型（`CustomOutbound.type` 判别字段）。 */
export type OutboundProtocolType = OutboundProtocol["type"];

/** 支持的协议类型（对齐 Rust `OutboundProtocol` 变体）。 */
export const OUTBOUND_PROTOCOL_OPTIONS: { value: OutboundProtocolType; label: string }[] = [
  { value: "vless", label: "VLESS" },
  { value: "vmess", label: "VMess" },
  { value: "shadowsocks", label: "Shadowsocks" },
  { value: "trojan", label: "Trojan" },
  { value: "hysteria2", label: "Hysteria2" },
  { value: "selector", label: "Selector（手动选择）" },
  { value: "urltest", label: "URLTest（自动测速）" },
];

const PROTOCOL_LABELS: Record<OutboundProtocolType, string> = {
  vless: "VLESS",
  vmess: "VMess",
  shadowsocks: "Shadowsocks",
  trojan: "Trojan",
  hysteria2: "Hysteria2",
  selector: "Selector",
  urltest: "URLTest",
};

/** 协议类型显示名（列表卡片标签）。 */
export function outboundProtocolLabel(type: OutboundProtocolType): string {
  return PROTOCOL_LABELS[type];
}

/** 协议是否为分组出站（selector / urltest；镜像 Rust `OutboundProtocol::is_group`）。 */
export function isGroupProtocol(type: OutboundProtocolType): type is "selector" | "urltest" {
  return type === "selector" || type === "urltest";
}

/** V2Ray 传输类型选项（对齐 `OutboundTransport.kind`）。 */
export const OUTBOUND_TRANSPORT_OPTIONS: { value: string; label: string }[] = [
  { value: "tcp", label: "TCP（无额外传输）" },
  { value: "ws", label: "WebSocket" },
  { value: "grpc", label: "gRPC" },
  { value: "http", label: "HTTP" },
  { value: "httpupgrade", label: "HTTP Upgrade" },
];

/** Shadowsocks 常见加密方式。 */
export const SHADOWSOCKS_METHOD_OPTIONS: { value: string; label: string }[] = [
  { value: "2022-blake3-aes-128-gcm", label: "2022-blake3-aes-128-gcm" },
  { value: "2022-blake3-aes-256-gcm", label: "2022-blake3-aes-256-gcm" },
  { value: "2022-blake3-chacha20-poly1305", label: "2022-blake3-chacha20-poly1305" },
  { value: "aes-256-gcm", label: "aes-256-gcm" },
  { value: "aes-128-gcm", label: "aes-128-gcm" },
  { value: "chacha20-ietf-poly1305", label: "chacha20-ietf-poly1305" },
  { value: "none", label: "none" },
];

/** VMess 加密方式。 */
export const VMESS_SECURITY_OPTIONS: { value: string; label: string }[] = [
  { value: "auto", label: "auto" },
  { value: "none", label: "none" },
  { value: "zero", label: "zero" },
  { value: "aes-128-gcm", label: "aes-128-gcm" },
  { value: "chacha20-poly1305", label: "chacha20-poly1305" },
];

/** Hysteria2 obfs 混淆类型（空 = 不使用）。 */
export const HYSTERIA2_OBFS_OPTIONS: { value: string; label: string }[] = [
  { value: "", label: "不使用" },
  { value: "salamander", label: "salamander" },
  { value: "gecko", label: "gecko" },
];

/** 传输类型是否需要 path / service_name 字段。 */
export function transportUsesPath(kind: string): boolean {
  return kind !== "" && kind !== "tcp";
}

/** 传输类型是否需要 host / Host 头字段（gRPC 无 host）。 */
export function transportUsesHost(kind: string): boolean {
  return kind === "ws" || kind === "http" || kind === "httpupgrade";
}

/** path 字段标签（gRPC 语义为 service_name）。 */
export function transportPathLabel(kind: string): string {
  return kind === "grpc" ? "service name" : "path";
}
