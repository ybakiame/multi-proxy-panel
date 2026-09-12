import type {
  ConfigSlices,
  DnsMatchType,
  DnsMode,
  DnsRule,
  DnsRuleAction,
  DnsServer,
  DnsServerType,
  DnsSlice,
  DnsStrategy,
} from "@pp/client-core";

/**
 * DNS 切片（ADR-0005 P0-4b）纯逻辑：枚举选项 / 标签 / 结构守卫 / 表单校验。
 *
 * 与页面组件分离，便于单测与复用；校验规则对齐 ADR §3.5（D5 前端表单即时校验）：
 * tag 必填唯一无空白、非 local 类型 server 必填、端口 1-65535、takeover 且切片启用时
 * final_tag 必填且指向已定义 server、DNS 规则的 server_tag 必须指向已定义 server。
 */

// ---------------------------------------------------------------------------
// 枚举选项
// ---------------------------------------------------------------------------

/** DNS 模式选项（仅 Android 展示；桌面恒 takeover 语义，不渲染该行）。 */
export const DNS_MODE_OPTIONS: { value: DnsMode; label: string }[] = [
  { value: "follow_system", label: "跟随系统" },
  { value: "takeover", label: "接管" },
];

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

/** 匹配目标输入占位（按 match_type 语义变化）。 */
export const DNS_TARGET_PLACEHOLDER: Record<DnsMatchType, string> = {
  domain: "例如：example.com",
  domain_suffix: "例如：google.com",
  domain_keyword: "例如：google",
  rule_set: "规则集 tag",
  query_type: "如 A,AAAA",
};

// ---------------------------------------------------------------------------
// 结构守卫
// ---------------------------------------------------------------------------

/**
 * `ConfigSlices` 结构守卫：`CONFIG_SLICES_KEY` 缓存形态异常时视为未加载，
 * 页面渲染空态而非访问 undefined 崩溃（对齐 localOverrideGuards 的第二道防线）。
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
    Array.isArray(slices.dns.rules)
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

/** DNS 分流规则摘要（列表卡片副标题）。 */
export function dnsRuleSummary(rule: DnsRule): string {
  const target = `${dnsMatchTypeLabel(rule.match_type)}: ${rule.target}`;
  if (rule.action === "predefined") {
    const rcode = rule.rcode.trim() !== "" ? rule.rcode.trim().toUpperCase() : DEFAULT_DNS_RCODE;
    return `${target} → 预定义应答 ${rcode}`;
  }
  if (rule.action === "reject") {
    return `${target} → 拒绝`;
  }
  return `${target} → ${rule.server_tag}`;
}

/** 已定义 server tag 的下拉选项（供规则的 server_tag 与 final_tag 复用）。 */
export function dnsServerTagOptions(servers: DnsServer[]): { value: string; label: string; description: string }[] {
  return servers
    .filter((server) => isTagValid(server.tag))
    .map((server) => ({
      value: server.tag.trim(),
      label: server.tag.trim(),
      description: dnsServerSummary(server),
    }));
}

// ---------------------------------------------------------------------------
// 校验
// ---------------------------------------------------------------------------

/** 端口草稿解析：空串 → null（使用核心默认端口）；非法 → 错误文案。 */
export function parsePortDraft(raw: string): { value: number | null; error: string | null } {
  const trimmed = raw.trim();
  if (trimmed === "") {
    return { value: null, error: null };
  }
  if (!/^\d+$/.test(trimmed)) {
    return { value: null, error: "端口需为 1-65535 的整数" };
  }
  const value = Number(trimmed);
  if (value < 1 || value > 65535) {
    return { value: null, error: "端口需在 1-65535 之间" };
  }
  return { value, error: null };
}

/** 应答码是否合法（大小写不敏感，对齐 Rust `is_valid_rcode`）。 */
export function isValidRcode(value: string): boolean {
  return DNS_RCODE_OPTIONS.some((option) => option.value === value.trim().toUpperCase());
}

/** 宽松 CIDR 校验：空串合法（核心默认）；非空须含非空地址与 `/` 前缀（对齐 Rust `validate_cidr_loose`）。 */
export function isCidrLoose(value: string): boolean {
  const trimmed = value.trim();
  if (trimmed === "") {
    return true;
  }
  const slash = trimmed.indexOf("/");
  return slash > 0 && slash < trimmed.length - 1;
}

/** 服务器表单字段错误（`null` = 合法）。 */
export interface DnsServerFormErrors {
  tag: string | null;
  server: string | null;
  port: string | null;
  inet4: string | null;
  inet6: string | null;
}

/**
 * 校验服务器表单草稿。`otherTags` 为除自身外的已有 tag（编辑时排除自身），
 * 用于唯一性校验。
 */
export function validateDnsServerForm(
  fields: {
    tag: string;
    serverType: DnsServerType;
    server: string;
    port: string;
    inet4Range: string;
    inet6Range: string;
  },
  otherTags: readonly string[],
): DnsServerFormErrors {
  const trimmedTag = fields.tag.trim();
  let tagError: string | null = null;
  if (trimmedTag === "") {
    tagError = "请输入 tag";
  } else if (/\s/.test(fields.tag)) {
    tagError = "tag 不能包含空白字符";
  } else if (otherTags.includes(trimmedTag)) {
    tagError = "tag 已存在，请保持唯一";
  }

  const usesServer = fields.serverType !== "local" && fields.serverType !== "fakeip";
  const serverError = usesServer && fields.server.trim() === "" ? "请输入服务器地址" : null;
  const isFakeip = fields.serverType === "fakeip";

  return {
    tag: tagError,
    server: serverError,
    port: parsePortDraft(fields.port).error,
    inet4: isFakeip && !isCidrLoose(fields.inet4Range) ? "需为 CIDR 网段，如 198.18.0.0/15" : null,
    inet6: isFakeip && !isCidrLoose(fields.inet6Range) ? "需为 CIDR 网段，如 fd00::/8" : null,
  };
}

/** 整切片校验结果（页面保存按钮据此禁用）。 */
export interface DnsSliceErrors {
  /** 与 `dns.servers` 等长，逐项首个错误（`null` = 合法）。 */
  serverErrors: (string | null)[];
  /** 与 `dns.rules` 等长，逐项首个错误（`null` = 合法）。 */
  ruleErrors: (string | null)[];
  finalTag: string | null;
}

/** 校验整个 DNS 切片草稿（保存前调用）。 */
export function validateDnsSlice(dns: DnsSlice): DnsSliceErrors {
  const serverTags = new Set<string>();

  const serverErrors = dns.servers.map((server) => {
    if (!isTagValid(server.tag)) {
      return "tag 不能为空且不含空白";
    }
    if (serverTags.has(server.tag.trim())) {
      return "tag 重复";
    }
    serverTags.add(server.tag.trim());
    const usesServer = server.server_type !== "local" && server.server_type !== "fakeip";
    if (usesServer && server.server.trim() === "") {
      return "非 local/FakeIP 类型必须填写服务器地址";
    }
    if (server.server_type === "fakeip") {
      if (!isCidrLoose(server.inet4_range)) {
        return "inet4_range 需为 CIDR 网段";
      }
      if (!isCidrLoose(server.inet6_range)) {
        return "inet6_range 需为 CIDR 网段";
      }
    }
    if (server.server_port !== null && (server.server_port < 1 || server.server_port > 65535)) {
      return "端口需在 1-65535 之间";
    }
    return null;
  });

  const ruleErrors = dns.rules.map((rule) => {
    if (rule.target.trim() === "") {
      return "匹配目标不能为空";
    }
    if (rule.match_type === "query_type") {
      const invalid = firstInvalidQueryType(rule.target);
      if (invalid !== null) {
        return invalid === "" ? "查询类型项不能为空" : `查询类型「${invalid}」无效`;
      }
    }
    if (rule.action === "predefined") {
      if (rule.server_tag.trim() !== "") {
        return "预定义应答不能指定目标服务器";
      }
      if (rule.rcode.trim() !== "" && !isValidRcode(rule.rcode)) {
        return "应答码无效";
      }
      return null;
    }
    if (rule.action === "reject") {
      if (rule.server_tag.trim() !== "") {
        return "拒绝动作不能指定目标服务器";
      }
      return null;
    }
    if (rule.server_tag.trim() === "") {
      return "请选择目标 DNS 服务器";
    }
    if (!serverTags.has(rule.server_tag.trim())) {
      return "目标 DNS 服务器不存在";
    }
    return null;
  });

  let finalTag: string | null = null;
  const trimmedFinalTag = dns.final_tag.trim();
  if (dns.mode === "takeover" && dns.enabled && trimmedFinalTag === "") {
    finalTag = "接管模式下必须选择 final 服务器";
  } else if (trimmedFinalTag !== "" && !serverTags.has(trimmedFinalTag)) {
    finalTag = "final 服务器不存在";
  }

  return { serverErrors, ruleErrors, finalTag };
}

/** 校验结果是否全部通过。 */
export function isDnsSliceValid(errors: DnsSliceErrors): boolean {
  return (
    errors.serverErrors.every((error) => error === null) &&
    errors.ruleErrors.every((error) => error === null) &&
    errors.finalTag === null
  );
}
