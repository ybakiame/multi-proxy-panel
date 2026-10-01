import { formatRuleSetTarget } from "./rules";
import type {
  ConfigSlices,
  DnsMatchType,
  DnsRule,
  DnsRuleAction,
  DnsServer,
  DnsServerPreset,
  DnsServerType,
  DnsStrategy,
} from "./api";

/**
 * DNS 切片（ADR-0005 P0-4b）纯逻辑：枚举选项 / 标签 / 结构守卫 / 表单校验。
 *
 * 与页面组件分离，便于单测与复用；校验规则对齐 ADR §3.5（D5 前端表单即时校验）：
 * tag 必填唯一无空白、非 local 类型 server 必填、端口 1-65535、takeover 时
 * final_tag 必填且指向已定义 server、DNS 规则的 server_tag 必须指向已定义 server。
 */

// ---------------------------------------------------------------------------
// 枚举选项
// ---------------------------------------------------------------------------

/** DNS 服务器类型选项（`local` / `fakeip` 无需 server / port）。 */
export const DNS_SERVER_TYPE_OPTIONS: { value: DnsServerType; label: string }[] = [
  { value: "udp", label: "UDP" },
  { value: "tls", label: "TLS" },
  { value: "https", label: "HTTPS" },
  { value: "quic", label: "QUIC" },
  { value: "h3", label: "HTTP/3" },
  { value: "local", label: "本地 (local)" },
  { value: "fakeip", label: "FakeIP" },
];

/** DNS 分流规则匹配类型选项。 */
export const DNS_MATCH_TYPE_OPTIONS: { value: DnsMatchType; label: string }[] = [
  { value: "domain", label: "域名" },
  { value: "domain_suffix", label: "域名后缀" },
  { value: "domain_keyword", label: "域名关键词" },
  { value: "rule_set", label: "规则集" },
  { value: "query_type", label: "查询类型" },
  { value: "clash_mode", label: "出站模式" },
];

/** `clash_mode` 匹配目标选项（渲染为字符串，对齐 sing-box DNS 规则 schema）。 */
export const DNS_CLASH_MODE_OPTIONS: { value: string; label: string }[] = [
  { value: "rule", label: "规则 (rule)" },
  { value: "global", label: "全局 (global)" },
  { value: "direct", label: "直连 (direct)" },
];

/** DNS 规则动作选项。 */
export const DNS_RULE_ACTION_OPTIONS: { value: DnsRuleAction; label: string }[] = [
  { value: "route", label: "路由到服务器" },
  { value: "predefined", label: "预定义应答" },
  { value: "reject", label: "拒绝" },
];

/** `predefined` 动作的应答码选项（对齐 Rust `DNS_RCODE_NAMES`）。 */
export const DNS_RCODE_OPTIONS: { value: string; label: string }[] = [
  { value: "NOERROR", label: "NOERROR" },
  { value: "FORMERR", label: "FORMERR" },
  { value: "SERVFAIL", label: "SERVFAIL" },
  { value: "NXDOMAIN", label: "NXDOMAIN" },
  { value: "NOTIMP", label: "NOTIMP" },
  { value: "REFUSED", label: "REFUSED" },
];

/** 默认应答码（Rust 渲染 `predefined` 且 rcode 为空时使用）。 */
export const DEFAULT_DNS_RCODE = "NOERROR";

/** FakeIP 默认 IPv4 网段（对齐 Rust `DEFAULT_FAKEIP_INET4_RANGE`）。 */
export const DEFAULT_FAKEIP_INET4_RANGE = "198.18.0.0/15";

/**
 * `query_type` 匹配目标允许的查询类型名（对齐 Rust `QUERY_TYPE_NAMES`）。
 *
 * 匹配大小写不敏感；渲染时 Rust 会归一化为大写。
 */
export const DNS_QUERY_TYPE_NAMES: readonly string[] = [
  "A",
  "NS",
  "CNAME",
  "SOA",
  "PTR",
  "MX",
  "TXT",
  "AAAA",
  "SRV",
  "NAPTR",
  "CAA",
  "TLSA",
  "DS",
  "DNSKEY",
  "RRSIG",
  "NSEC",
  "NSEC3",
  "SVCB",
  "HTTPS",
  "ANY",
  "OPT",
  "HINFO",
  "MINFO",
  "WKS",
  "AXFR",
  "IXFR",
];

/** 查询类型名是否合法（大小写不敏感，对齐 Rust `is_valid_query_type`）。 */
export function isValidQueryType(value: string): boolean {
  return DNS_QUERY_TYPE_NAMES.includes(value.trim().toUpperCase());
}

/** 逗号分隔的 `query_type` 目标逐项校验；返回首个非法项（`null` = 全部合法）。 */
export function firstInvalidQueryType(target: string): string | null {
  for (const item of target.split(",")) {
    const trimmed = item.trim();
    if (trimmed === "" || !isValidQueryType(trimmed)) {
      return trimmed === "" ? item : trimmed;
    }
  }
  return null;
}

/** 全局解析策略选项。 */
export const DNS_STRATEGY_OPTIONS: { value: DnsStrategy; label: string }[] = [
  { value: "prefer_ipv4", label: "优先 IPv4" },
  { value: "prefer_ipv6", label: "优先 IPv6" },
  { value: "ipv4_only", label: "仅 IPv4" },
  { value: "ipv6_only", label: "仅 IPv6" },
];

const DNS_MATCH_TYPE_LABELS: Record<DnsMatchType, string> = {
  domain: "域名",
  domain_suffix: "域名后缀",
  domain_keyword: "域名关键词",
  rule_set: "规则集",
  query_type: "查询类型",
  clash_mode: "出站模式",
};

const DNS_SERVER_TYPE_LABELS: Record<DnsServerType, string> = {
  udp: "UDP",
  tls: "TLS",
  https: "HTTPS",
  quic: "QUIC",
  h3: "HTTP/3",
  local: "本地",
  fakeip: "FakeIP",
};

const DNS_RULE_ACTION_LABELS: Record<DnsRuleAction, string> = {
  route: "路由",
  predefined: "预定义应答",
  reject: "拒绝",
};

const DNS_STRATEGY_LABELS: Record<DnsStrategy, string> = {
  prefer_ipv4: "优先 IPv4",
  prefer_ipv6: "优先 IPv6",
  ipv4_only: "仅 IPv4",
  ipv6_only: "仅 IPv6",
};

export function dnsMatchTypeLabel(value: DnsMatchType): string {
  return DNS_MATCH_TYPE_LABELS[value] ?? value;
}

export function dnsServerTypeLabel(value: DnsServerType): string {
  return DNS_SERVER_TYPE_LABELS[value] ?? value;
}

export function dnsStrategyLabel(value: DnsStrategy): string {
  return DNS_STRATEGY_LABELS[value] ?? value;
}

export function dnsRuleActionLabel(value: DnsRuleAction): string {
  return DNS_RULE_ACTION_LABELS[value] ?? value;
}

/** 匹配目标输入占位（按 match_type 语义变化；`clash_mode` 走选择器，占位不用）。 */
export const DNS_TARGET_PLACEHOLDER: Record<DnsMatchType, string> = {
  domain: "例如：example.com",
  domain_suffix: "例如：google.com",
  domain_keyword: "例如：google",
  rule_set: "规则集 tag",
  query_type: "如 A,AAAA",
  clash_mode: "",
};

// ---------------------------------------------------------------------------
// 内置角色服务器（local / proxy）
// ---------------------------------------------------------------------------

/** 内置默认 DNS 的角色服务器 tag：`local`（境内直连解析）/ `proxy`（境外解析，经代理出站）。 */
export type DnsRoleTag = "local" | "proxy";

/** tag 是否为内置角色服务器（`local` / `proxy` 两行；大小写敏感，对齐切片 schema）。 */
export function dnsRoleTag(tag: string): DnsRoleTag | null {
  const trimmed = tag.trim();
  if (trimmed === "local") {
    return "local";
  }
  if (trimmed === "proxy") {
    return "proxy";
  }
  return null;
}

/** 角色服务器的行内说明（列表副标题前缀）。 */
export const DNS_ROLE_LABELS: Record<DnsRoleTag, string> = {
  local: "直连 DNS",
  proxy: "代理 DNS",
};

/** 角色服务器可更换的预置目录分组：local 取内置 + 境内，proxy 取内置 + 境外。 */
export const DNS_ROLE_PRESET_GROUPS: Record<DnsRoleTag, readonly DnsServerPreset["group"][]> = {
  local: ["builtin", "cn"],
  proxy: ["builtin", "global"],
};

// ---------------------------------------------------------------------------
// 结构守卫
// ---------------------------------------------------------------------------

/**
 * `ConfigSlices` 结构守卫：`CONFIG_SLICES_KEY` 缓存形态异常时视为未加载，
 * 页面渲染空态而非访问 undefined 崩溃（对齐 localOverrideGuards 的第二道防线）。
 *
 * 同时校验 DNS 与出站两个切片（二者同文件存取，损坏语义一致；原移动端在
 * dnsSlice / outboundForm 各有一份仅查本段的同名守卫，上移后合并为单一实现）。
 */
export function isConfigSlices(value: unknown): value is ConfigSlices {
  if (!value || typeof value !== "object") {
    return false;
  }
  const slices = value as ConfigSlices;
  return (
    typeof slices.dns === "object" &&
    slices.dns !== null &&
    Array.isArray(slices.dns.servers) &&
    Array.isArray(slices.dns.rules) &&
    typeof slices.outbounds === "object" &&
    slices.outbounds !== null &&
    Array.isArray(slices.outbounds.items)
  );
}

// ---------------------------------------------------------------------------
// 标签 / 摘要
// ---------------------------------------------------------------------------

/** tag 规则：trim 后非空且不含任何空白字符。 */
export function isTagValid(tag: string): boolean {
  return tag.trim().length > 0 && !/\s/.test(tag);
}

/** DNS 服务器摘要（列表卡片副标题）。 */
export function dnsServerSummary(server: DnsServer): string {
  if (server.server_type === "local") {
    return "使用系统本地 DNS";
  }
  if (server.server_type === "fakeip") {
    const inet4 = server.inet4_range.trim() !== "" ? server.inet4_range.trim() : DEFAULT_FAKEIP_INET4_RANGE;
    const parts = [`虚拟网段 ${inet4}`];
    if (server.inet6_range.trim() !== "") {
      parts.push(server.inet6_range.trim());
    }
    return parts.join(" · ");
  }
  const address = server.server_port !== null ? `${server.server}:${server.server_port}` : server.server;
  const parts = [address];
  if (server.detour.trim() !== "") {
    parts.push(`出站 ${server.detour}`);
  }
  if (server.domain_resolver.trim() !== "") {
    parts.push(`解析器 ${server.domain_resolver}`);
  }
  return parts.join(" · ");
}

/** DNS 分流规则摘要（列表卡片副标题；rule_set 多 tag 展示为 `a + b`）。 */
export function dnsRuleSummary(rule: DnsRule): string {
  const displayTarget = rule.match_type === "rule_set" ? formatRuleSetTarget(rule.target) : rule.target;
  const target = `${dnsMatchTypeLabel(rule.match_type)}: ${displayTarget}`;
  if (rule.action === "predefined") {
    const rcode = rule.rcode.trim() !== "" ? rule.rcode.trim().toUpperCase() : DEFAULT_DNS_RCODE;
    return `${target} → 预定义应答 ${rcode}`;
  }
  if (rule.action === "reject") {
    return `${target} → 拒绝`;
  }
  return `${target} → ${rule.server_tag}`;
}

/** 已定义且**启用中** server tag 的下拉选项（供规则的 server_tag 与 final_tag 复用）；
 * 弃用（enabled=false）服务器不渲染进运行配置，不可作为引用目标。 */
export function dnsServerTagOptions(servers: DnsServer[]): { value: string; label: string; description: string }[] {
  return servers
    .filter((server) => server.enabled && isTagValid(server.tag))
    .map((server) => ({
      value: server.tag.trim(),
      label: server.tag.trim(),
      description: dnsServerSummary(server),
    }));
}
