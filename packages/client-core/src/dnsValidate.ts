/**
 * DNS 切片校验与比较（自 dnsSlice.ts 拆分，遵守文件规模门禁）。
 *
 * 校验规则对齐 ADR-0005 §3.5（D5 前端表单即时校验）与 Rust 侧 `validate`：
 * tag 必填唯一无空白、非 local 类型 server 必填、端口 1-65535、takeover 时
 * final_tag 必填且指向已定义 server、DNS 规则的 server_tag 必须指向已定义 server。
 */

import type { DnsSlice, DnsServerType } from "./api";
import { parseRuleSetTags } from "./rules";
import { DNS_CLASH_MODE_OPTIONS, DNS_RCODE_OPTIONS, firstInvalidQueryType, isTagValid } from "./dnsSlice";

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

  const enabledTags = new Set(dns.servers.filter((server) => server.enabled).map((server) => server.tag.trim()));

  const ruleErrors = dns.rules.map((rule) => {
    if (rule.target.trim() === "") {
      return "匹配目标不能为空";
    }
    if (rule.match_type === "rule_set" && parseRuleSetTags(rule.target).length === 0) {
      return "请至少选择一个规则集";
    }
    if (rule.match_type === "clash_mode" && !DNS_CLASH_MODE_OPTIONS.some((o) => o.value === rule.target.trim())) {
      return "出站模式须为 rule / global / direct";
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
    if (!enabledTags.has(rule.server_tag.trim())) {
      return "目标 DNS 服务器已弃用";
    }
    return null;
  });

  // 编辑即接管语义：只要切片有正文（servers 非空），保存即落为 takeover，
  // final 因此恒必填（对齐 Rust 侧 takeover 校验）。
  let finalTag: string | null = null;
  const trimmedFinalTag = dns.final_tag.trim();
  if ((dns.mode === "takeover" || dns.servers.length > 0) && trimmedFinalTag === "") {
    finalTag = "必须选择 final 服务器（保存后按自定义 DNS 接管生效）";
  } else if (trimmedFinalTag !== "" && !serverTags.has(trimmedFinalTag)) {
    finalTag = "final 服务器不存在";
  } else if (trimmedFinalTag !== "" && !enabledTags.has(trimmedFinalTag)) {
    finalTag = "final 服务器已弃用";
  }

  return { serverErrors, ruleErrors, finalTag };
}

/**
 * DNS 切片内容深比较（字段顺序稳定：草稿与内置视图同源构建，JSON 序列化即可靠）。
 * 用于「编辑即接管」判定：内容与内置默认一致 → 保持跟随系统（不落接管）。
 */
export function dnsSliceEquals(a: DnsSlice, b: DnsSlice): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** 校验结果是否全部通过。 */
export function isDnsSliceValid(errors: DnsSliceErrors): boolean {
  return (
    errors.serverErrors.every((error) => error === null) &&
    errors.ruleErrors.every((error) => error === null) &&
    errors.finalTag === null
  );
}
