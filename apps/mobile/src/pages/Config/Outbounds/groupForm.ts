import type { CustomOutbound, GroupOutbound } from "@pp/client-core";
import type { OutboundFormFields } from "./outboundForm";

/**
 * 分组出站（selector / urltest）表单辅助与校验（ADR-0005 P0-4c）。
 *
 * 成员候选来源与 Rust `validate_group_members` 保持一致：静态订阅节点
 * （`subscription_node_tags`，订阅缓存，不依赖核心运行）+ 切片节点出站（enabled）+
 * 内置 `direct` / `block`；**不含其它分组**（v1 禁嵌套，故内置 proxy / auto 分组不作
 * 候选）。
 * 校验对齐 Rust：成员非空、selector default ∈ 成员；urltest 的 url / interval /
 * tolerance 为宽松格式校验（Rust 侧不校验，前端只拦截明显非法值）。
 */

/** sing-box urltest 默认测速 URL（与 Rust `default_urltest_url` 一致）。 */
export const DEFAULT_URLTEST_URL = "https://www.gstatic.com/generate_204";

/** 时长格式（数字 + 单位，如 `3m` / `30s` / `1h`）。 */
const DURATION_PATTERN = /^\d+(\.\d+)?(ns|us|ms|s|m|h|d)$/;

/** 分组成员候选项（tag + 友好名）。 */
export interface GroupMemberCandidate {
  /** 写入 `outbounds` / `default` 的 tag。 */
  value: string;
  /** 列表行显示名（订阅节点名 / 切片出站名 / direct / block）。 */
  label: string;
  /** 可选来源说明。 */
  hint?: string;
}

/** 成员候选数据源。 */
export interface GroupMemberSources {
  /** 静态订阅节点名（`subscription_node_tags`，订阅缓存，不依赖核心运行）。 */
  subscriptionNodes: readonly string[];
  /** 切片节点出站（enabled 且非分组）。 */
  sliceNodes: readonly { name: string; tag: string }[];
}

/**
 * 构建分组成员候选：静态订阅节点 + 切片节点 + 内置 direct / block，按 tag 去重。
 *
 * 不接受分组（含模板出站分组 proxy / auto）作为候选，与 Rust「v1 禁嵌套分组」校验一致。
 */
export function buildGroupMemberCandidates(sources: GroupMemberSources): GroupMemberCandidate[] {
  const options: GroupMemberCandidate[] = [];
  const seen = new Set<string>();
  const push = (value: string, label: string, hint?: string) => {
    const tag = value.trim();
    if (tag === "" || seen.has(tag)) return;
    seen.add(tag);
    options.push({ value: tag, label, hint });
  };
  for (const name of sources.subscriptionNodes) push(name, name, "订阅节点");
  for (const node of sources.sliceNodes) push(node.tag, node.name.trim() || node.tag, node.tag);
  push("direct", "direct", "内置直连");
  push("block", "block", "内置拦截");
  return options;
}

/** 分组出站 → 表单草稿（编辑预填；调用方已建好基础草稿）。 */
export function applyGroupToForm(form: OutboundFormFields, item: GroupOutbound): void {
  form.members = [...item.outbounds];
  form.interruptExistConnections = item.interrupt_exist_connections;
  if (item.type === "selector") {
    form.groupDefault = item.default;
  } else {
    form.groupUrl = item.url;
    form.groupInterval = item.interval;
    form.groupTolerance = String(item.tolerance);
  }
}

/** 表单草稿 → 分组出站（保存转换）。 */
export function groupFormToOutbound(
  fields: OutboundFormFields,
  base: { id: string; name: string; enabled: boolean },
): CustomOutbound {
  const outbounds = [...fields.members];
  const interrupt_exist_connections = fields.interruptExistConnections;
  if (fields.protocol === "selector") {
    return { ...base, type: "selector", outbounds, default: fields.groupDefault, interrupt_exist_connections };
  }
  const tolerance = Number(fields.groupTolerance.trim());
  return {
    ...base,
    type: "urltest",
    outbounds,
    url: fields.groupUrl.trim(),
    interval: fields.groupInterval.trim(),
    tolerance: Number.isFinite(tolerance) && tolerance >= 0 ? Math.trunc(tolerance) : 0,
    interrupt_exist_connections,
  };
}

/** 分组出站摘要（列表卡片副标题）：成员数 + 前几个 tag。 */
export function groupSummary(item: GroupOutbound): string {
  const preview = item.outbounds.slice(0, 3).join(", ");
  const suffix = item.outbounds.length > 3 ? "…" : "";
  return `${item.outbounds.length} 个成员 · ${preview}${suffix}`;
}

/** 分组表单字段错误（`null` = 合法）。 */
export interface GroupFormErrors {
  members: string | null;
  defaultMember: string | null;
  url: string | null;
  interval: string | null;
  tolerance: string | null;
}

/** 内置分组的即时校验：仅 urltest 的 url / interval / tolerance 格式。 */
function validateBuiltinGroupFields(fields: OutboundFormFields): GroupFormErrors {
  let urlError: string | null = null;
  let intervalError: string | null = null;
  let toleranceError: string | null = null;
  if (fields.protocol === "urltest") {
    const url = fields.groupUrl.trim();
    if (url !== "" && !/^https?:\/\/\S+$/i.test(url)) {
      urlError = "URL 需以 http:// 或 https:// 开头";
    }
    const interval = fields.groupInterval.trim();
    if (interval !== "" && !DURATION_PATTERN.test(interval)) {
      intervalError = "格式如 3m / 30s / 1h";
    }
    const tolerance = fields.groupTolerance.trim();
    if (tolerance !== "") {
      if (!/^\d+$/.test(tolerance)) {
        toleranceError = "请输入 0-65535 的整数";
      } else if (Number(tolerance) > 65535) {
        toleranceError = "范围 0-65535";
      }
    }
  }
  return { members: null, defaultMember: null, url: urlError, interval: intervalError, tolerance: toleranceError };
}

/**
 * 校验分组表单草稿（即时，与 Rust 校验相容）。
 *
 * 成员非空、selector default ∈ 成员；urltest 的 url 需形如 http(s)://…、
 * interval 需形如数字+单位、tolerance 需为 0-65535 整数。空 url / interval
 * 表示使用核心默认值，合法。
 */
export function validateGroupFields(fields: OutboundFormFields): GroupFormErrors {
  // 内置动态分组（proxy/auto）：成员模板计算，仅可调字段参与校验；default 悬空由后端
  // apply 兜底。内置静态分组（global/final）走完整校验（成员可编辑、非空、防循环由
  // 后端把关）。
  if (fields.builtin && !fields.builtinMembersEditable) {
    return validateBuiltinGroupFields(fields);
  }
  const membersError = fields.members.length === 0 ? "请至少选择一个成员" : null;

  let defaultError: string | null = null;
  if (fields.protocol === "selector" && fields.groupDefault !== "" && !fields.members.includes(fields.groupDefault)) {
    defaultError = "默认成员必须是已选成员";
  }

  let urlError: string | null = null;
  let intervalError: string | null = null;
  let toleranceError: string | null = null;
  if (fields.protocol === "urltest") {
    const url = fields.groupUrl.trim();
    if (url !== "" && !/^https?:\/\/\S+$/i.test(url)) {
      urlError = "URL 需以 http:// 或 https:// 开头";
    }
    const interval = fields.groupInterval.trim();
    if (interval !== "" && !DURATION_PATTERN.test(interval)) {
      intervalError = "格式如 3m / 30s / 1h";
    }
    const tolerance = fields.groupTolerance.trim();
    if (tolerance !== "") {
      if (!/^\d+$/.test(tolerance)) {
        toleranceError = "请输入 0-65535 的整数";
      } else {
        const value = Number(tolerance);
        if (value > 65535) {
          toleranceError = "范围 0-65535";
        }
      }
    }
  }

  return {
    members: membersError,
    defaultMember: defaultError,
    url: urlError,
    interval: intervalError,
    tolerance: toleranceError,
  };
}

/** 分组表单校验结果是否全部通过。 */
export function isGroupFormValid(errors: GroupFormErrors): boolean {
  return (
    errors.members === null &&
    errors.defaultMember === null &&
    errors.url === null &&
    errors.interval === null &&
    errors.tolerance === null
  );
}

/**
 * 校验单个分组条目的切片级约束（保存前整切片校验调用）。
 *
 * 成员非空、`slice-` 前缀成员必须指向仍启用存在的切片节点（悬空引用提示）、
 * selector default ∈ 成员，对齐 Rust `validate_group_members`。
 */
export function validateGroupItem(item: GroupOutbound, enabledNodeTags: ReadonlySet<string>): string | null {
  if (item.outbounds.length === 0) {
    return "请至少选择一个成员";
  }
  const dangling = item.outbounds.find((member) => member.startsWith("slice-") && !enabledNodeTags.has(member));
  if (dangling) {
    return `成员「${dangling}」对应的出站已删除或未启用`;
  }
  if (item.type === "selector" && item.default !== "" && !item.outbounds.includes(item.default)) {
    return "默认成员必须是已选成员";
  }
  return null;
}
